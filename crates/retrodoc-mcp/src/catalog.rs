//! The read-only tools as data: their names, descriptions and argument schemas, and a call by name
//! with JSON arguments. The MCP server and the `ask` agent (ADR 0047) offer the same tools from
//! here, so both answer the same thing to the same call.

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::source::DEFAULT_LOG;
use crate::tools::Docs;

/// Hits returned by `search_docs` when the caller gives no limit, and the most it can ask for.
pub const DEFAULT_LIMIT: usize = 5;
pub const MAX_LIMIT: usize = 20;

pub const LIST_DOMAINS: &str = "List the functional domains of the application with a description \
     and a confidence score. Start here to find your way.";
pub const GET_DOMAIN: &str =
    "Show one domain (e.g. `billing`) and its features and business rules.";
pub const GET_FEATURE: &str = "Show one feature (e.g. `billing/pay-invoice`): what it does, the \
     files it rests on, its use cases and its business rules (what is allowed, refused, limited, \
     with their evidence).";
pub const GET_USE_CASE: &str = "Show one use case (e.g. `billing/pay-invoice/pay-by-card`): who \
     does what and why, the steps, and the code each step cites. A low confidence means the claim \
     is not backed by the code: verify before relying on it. Lists the business rules that apply \
     to it.";
pub const READ_SOURCE: &str = "Show lines of a source file that the docs cite, with line numbers, \
     to check a claim against the code. Only cited files can be read; use the paths given by \
     get_feature and get_use_case.";
pub const GIT_LOG: &str = "Show the recent commits (hash, date, subject) that changed a source \
     file the docs cite, to see how and why a behavior evolved, across its renames (the previous \
     names are history only, not readable). Only cited files.";
pub const SEARCH_DOCS: &str = "Search the generated documentation (including business rules) and \
     the project's own docs with the words of a question. Answers `Not documented` when nothing \
     matches: say so rather than guess.";

#[derive(Deserialize, JsonSchema)]
pub(crate) struct NoParams {}

#[derive(Deserialize, JsonSchema)]
pub(crate) struct IdParams {
    /// The id as shown by the other tools (e.g. `billing/pay-invoice`).
    pub id: String,
}

#[derive(Deserialize, JsonSchema)]
pub(crate) struct SourceParams {
    /// A file cited by the docs, relative to the repository root (e.g. `app/invoice.rb`).
    pub path: String,
    /// First line to show, 1-based (default 1).
    pub start_line: Option<usize>,
    /// Last line to show, inclusive (default: 200 lines from `start_line`, at most 400).
    pub end_line: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
pub(crate) struct LogParams {
    /// A file cited by the docs, relative to the repository root.
    pub path: String,
    /// Maximum number of commits (default 10, at most 50).
    pub limit: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
pub(crate) struct SearchParams {
    /// What to look for, in the application's own words.
    pub query: String,
    /// Maximum number of results (default 5, at most 20).
    pub limit: Option<usize>,
}

/// One read-only tool: its name, what it does, and the JSON schema of its arguments (an object).
#[derive(Debug, Clone)]
pub struct ReadTool {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: serde_json::Value,
}

/// The seven read-only tools, in the order an agent finds its way with them. No tool that writes.
#[must_use]
pub fn read_tools() -> Vec<ReadTool> {
    vec![
        tool::<NoParams>("list_domains", LIST_DOMAINS),
        tool::<IdParams>("get_domain", GET_DOMAIN),
        tool::<IdParams>("get_feature", GET_FEATURE),
        tool::<IdParams>("get_use_case", GET_USE_CASE),
        tool::<SearchParams>("search_docs", SEARCH_DOCS),
        tool::<SourceParams>("read_source", READ_SOURCE),
        tool::<LogParams>("git_log", GIT_LOG),
    ]
}

/// The names of the read-only tools, for the error of an unknown one.
fn tool_names() -> String {
    read_tools()
        .iter()
        .map(|t| t.name)
        .collect::<Vec<_>>()
        .join(", ")
}

fn tool<P: JsonSchema>(name: &'static str, description: &'static str) -> ReadTool {
    let mut parameters = schemars::schema_for!(P).to_value();
    if let Some(object) = parameters.as_object_mut() {
        object.remove("$schema");
        object.remove("title");
        let properties = object
            .entry("properties")
            .or_insert_with(|| serde_json::json!({}));
        if let Some(properties) = properties.as_object_mut() {
            properties.values_mut().for_each(plain_property);
        }
    }
    ReadTool {
        name,
        description,
        parameters,
    }
}

/// One type and no `format` for a property: schemars gives an optional one the types
/// `["integer", "null"]` and an unsigned one `format: uint`, which some servers refuse. That it is
/// optional, `required` says already.
fn plain_property(property: &mut serde_json::Value) {
    let Some(property) = property.as_object_mut() else {
        return;
    };
    property.remove("format");
    let single = property
        .get("type")
        .and_then(serde_json::Value::as_array)
        .and_then(|types| types.iter().find(|t| *t != "null").cloned());
    if let Some(single) = single {
        property.insert("type".into(), single);
    }
}

/// Why a call could not be made: the tool is unknown or its arguments don't fit it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CallError {
    #[error("unknown tool `{0}`; the tools are {}", tool_names())]
    UnknownTool(String),
    #[error("invalid arguments for `{tool}`: {reason}")]
    InvalidArguments { tool: String, reason: String },
}

impl Docs {
    /// Answers the read-only tool `name` with `arguments` (a JSON object as text; empty or `null` is `{}`),
    /// with the defaults and the bounds of the MCP server. Reads files and git: call it off the
    /// async threads.
    ///
    /// # Errors
    ///
    /// Returns an error when the tool is unknown or the arguments are not an object it accepts.
    pub fn call_read_tool(&self, name: &str, arguments: &str) -> Result<String, CallError> {
        let arguments = if arguments.trim().is_empty() || arguments.trim() == "null" {
            "{}"
        } else {
            arguments
        };
        match name {
            "list_domains" => {
                parse::<NoParams>(name, arguments)?;
                Ok(self.list_domains())
            }
            "get_domain" => Ok(self.get_domain(&parse::<IdParams>(name, arguments)?.id)),
            "get_feature" => Ok(self.get_feature(&parse::<IdParams>(name, arguments)?.id)),
            "get_use_case" => Ok(self.get_use_case(&parse::<IdParams>(name, arguments)?.id)),
            "search_docs" => {
                let SearchParams { query, limit } = parse(name, arguments)?;
                Ok(self.search_docs(&query, search_limit(limit)))
            }
            "read_source" => {
                let SourceParams {
                    path,
                    start_line,
                    end_line,
                } = parse(name, arguments)?;
                Ok(self.read_source(&path, start_line, end_line))
            }
            "git_log" => {
                let LogParams { path, limit } = parse(name, arguments)?;
                Ok(self.git_log(&path, limit.unwrap_or(DEFAULT_LOG)))
            }
            _ => Err(CallError::UnknownTool(name.to_string())),
        }
    }
}

/// The number of hits of `search_docs`: [`DEFAULT_LIMIT`] when not given, within 1 and [`MAX_LIMIT`].
pub(crate) fn search_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

fn parse<P: DeserializeOwned>(tool: &str, arguments: &str) -> Result<P, CallError> {
    let value: serde_json::Value =
        serde_json::from_str(arguments).map_err(|e| CallError::InvalidArguments {
            tool: tool.to_string(),
            reason: format!("not JSON ({e})"),
        })?;
    if !value.is_object() {
        return Err(CallError::InvalidArguments {
            tool: tool.to_string(),
            reason: "not a JSON object".to_string(),
        });
    }
    serde_json::from_value(value).map_err(|e| CallError::InvalidArguments {
        tool: tool.to_string(),
        reason: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use retrodoc_pipeline::Glossary;

    use super::*;
    use crate::corpus::fixtures::*;

    fn docs() -> Docs {
        Docs::new(
            vec![domain("billing", "Billing", "Invoices and payments")],
            vec![feature(
                "pay-invoice",
                "Pay an invoice",
                "Settle what is due",
            )],
            vec![],
            &Glossary::default(),
            &[],
        )
    }

    // spec: ASK-19 (internal: the schemas are read from the catalog the agent offers)
    #[test]
    fn the_catalog_holds_the_seven_read_tools_each_with_an_object_schema() {
        let tools = read_tools();
        let names: Vec<&str> = tools.iter().map(|t| t.name).collect();
        assert_eq!(
            names,
            [
                "list_domains",
                "get_domain",
                "get_feature",
                "get_use_case",
                "search_docs",
                "read_source",
                "git_log"
            ]
        );
        for tool in &tools {
            assert_eq!(tool.parameters["type"], "object", "{}", tool.name);
            let text = tool.parameters.to_string();
            assert!(
                !text.contains("\"format\"") && !text.contains("\"null\""),
                "{text}"
            );
            assert!(tool.parameters["properties"].is_object(), "{}", tool.name);
            assert_ne!(tool.description, "");
        }
        let get_feature = &tools[2].parameters;
        assert_eq!(
            tools[5].parameters["properties"]["start_line"]["type"],
            "integer"
        );
        assert_eq!(get_feature["required"], serde_json::json!(["id"]));
    }

    #[test]
    fn a_call_by_name_answers_like_the_function_of_the_tool() {
        let docs = docs();
        assert_eq!(
            docs.call_read_tool("get_domain", r#"{"id":"billing"}"#),
            Ok(docs.get_domain("billing"))
        );
        assert_eq!(
            docs.call_read_tool("list_domains", ""),
            Ok(docs.list_domains())
        );
        assert_eq!(
            docs.call_read_tool("search_docs", r#"{"query":"invoice"}"#),
            Ok(docs.search_docs("invoice", DEFAULT_LIMIT))
        );
    }

    #[test]
    fn an_unknown_tool_or_bad_arguments_are_errors_that_name_the_cause() {
        let docs = docs();
        assert!(matches!(
            docs.call_read_tool("edit_section", "{}"),
            Err(CallError::UnknownTool(_))
        ));
        let missing = docs
            .call_read_tool("get_feature", r#"{"slug":"x"}"#)
            .unwrap_err();
        assert!(missing.to_string().contains("`id`"), "{missing}");
        let array = docs.call_read_tool("get_domain", "[1]").unwrap_err();
        assert!(array.to_string().contains("not a JSON object"), "{array}");
        assert!(docs.call_read_tool("get_domain", "{oops").is_err());
    }
}

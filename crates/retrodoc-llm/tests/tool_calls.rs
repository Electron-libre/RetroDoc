//! Tool calls through the provider wrappers (`specs/tool-calling.md`).

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use retrodoc_llm::{
    ChatMessage, CompletionRequest, CompletionResponse, HeartbeatProvider, LlmError, LlmProvider,
    Role, ToolCall, ToolSpec, Usage, UsageProvider, UsageTracker,
};

/// Asks for one tool call per round and records the tools each request offered.
#[derive(Clone, Default)]
struct CallingProvider {
    offered: Arc<Mutex<Vec<Vec<String>>>>,
}

#[async_trait]
impl LlmProvider for CallingProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let names = request.tools.iter().map(|t| t.name.clone()).collect();
        let mut offered = self.offered.lock().unwrap();
        offered.push(names);
        Ok(CompletionResponse {
            model: "m".to_string(),
            usage: Some(Usage {
                prompt_tokens: 100,
                completion_tokens: 10,
                cached_tokens: 0,
            }),
            tool_calls: vec![ToolCall {
                id: format!("call_{}", offered.len()),
                name: "list_domains".to_string(),
                arguments: "{}".to_string(),
            }],
            ..Default::default()
        })
    }
}

fn tool() -> ToolSpec {
    ToolSpec {
        name: "list_domains".to_string(),
        description: "List the domains".to_string(),
        parameters: serde_json::json!({"type": "object"}),
    }
}

// spec: TCL-8, TCL-9
#[tokio::test]
async fn each_round_of_a_tool_loop_passes_the_wrappers_and_is_counted() {
    let inner = CallingProvider::default();
    let tracker = UsageTracker::new();
    let provider = UsageProvider::new(HeartbeatProvider::new(inner.clone()), tracker.clone());
    tracker.set_pass("ask");

    let mut messages = vec![ChatMessage::new(Role::User, "Which domains?")];
    for _ in 0..3 {
        let response = provider
            .complete(CompletionRequest {
                messages: messages.clone(),
                tools: vec![tool()],
                ..Default::default()
            })
            .await
            .unwrap();
        let call = response.tool_calls[0].clone();
        assert_eq!(call.name, "list_domains");
        messages.push(ChatMessage::assistant_calls(
            response.content,
            vec![call.clone()],
        ));
        messages.push(ChatMessage::tool_result(call.id, "- Billing"));
    }

    assert_eq!(
        *inner.offered.lock().unwrap(),
        vec![vec!["list_domains"]; 3]
    );
    let total = tracker.report().total();
    assert_eq!(total.calls, 3);
    assert_eq!(total.prompt_tokens, 300);
    assert_eq!(total.completion_tokens, 30);
}

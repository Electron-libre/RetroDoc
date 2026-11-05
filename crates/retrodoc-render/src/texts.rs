//! The fixed text of the docs, one table per language (`specs/docs-language.md`, DLG-7): the
//! headings, labels and notes around the generated text. A language with no table keeps English.

use retrodoc_core::model::RuleKind;

use crate::edit_marks::EditedField;

/// The fixed text of the docs in one language. The labels that end a line before a value carry
/// their colon, written as the language writes it (`Confidence:`, `Confiance :`).
#[derive(Debug)]
pub struct Texts {
    pub confidence: &'static str,
    pub business_language: &'static str,
    pub not_scored: &'static str,
    /// Where the docs put what is not the core of the business: back-office tools, and use cases
    /// that read as code or are weakly backed by it.
    pub technical_heading: &'static str,
    /// What the section [`Texts::technical_heading`] holds, under it.
    pub technical_note: &'static str,
    pub features: &'static str,
    pub domain_overview: &'static str,
    pub use_cases: &'static str,
    pub no_use_case: &'static str,
    pub all_set_apart: &'static str,
    pub source_files: &'static str,
    pub primary_actor: &'static str,
    pub triggered_by: &'static str,
    pub what_happens: &'static str,
    /// The summary of the folded technical details, from the number of steps.
    pub technical_details: fn(usize) -> String,
    pub steps: &'static str,
    pub diagram: &'static str,
    pub human: &'static str,
    pub system: &'static str,
    pub code: &'static str,
    pub test: &'static str,
    pub documentation: &'static str,
    pub section: &'static str,
    pub validation: &'static str,
    pub permission: &'static str,
    pub state_transition: &'static str,
    pub limit: &'static str,
    pub computation: &'static str,
    pub business_rules: &'static str,
    pub entities: &'static str,
    pub rule_use_cases: &'static str,
    /// The heading and the introduction of the rules index, from the number of rules.
    pub rules_index_head: fn(usize) -> String,
    pub other_rules: &'static str,
    pub existing_documentation: &'static str,
    pub existing_doc: &'static str,
    pub states_otherwise: &'static str,
    /// The index for agents up to the heading of its domains, included.
    pub agent_index_head: &'static str,
    pub no_domain: &'static str,
    /// The rules section of the index for agents, from the link to the rules index and the number
    /// of rules.
    pub agent_index_rules: fn(&str, usize) -> String,
    pub agent_index_optional: &'static str,
    /// The mark of a section a person edited, before the fields it changes (SED-38, WEB-91).
    pub human_edit: &'static str,
    /// The name, the description and the narrative, as the mark of an edit names them.
    pub edited_fields: [&'static str; 3],
    /// Before the author of an edit.
    pub edited_by: &'static str,
}

impl Texts {
    pub const ENGLISH: Self = Self {
        confidence: "Confidence:",
        business_language: "Business language:",
        not_scored: "not scored",
        technical_heading: "Administration and technical",
        technical_note:
            "Back-office tools, and behaviours that read as code or that the code backs weakly.",
        features: "Features",
        domain_overview: "Domain overview",
        use_cases: "Use cases",
        no_use_case: "None could be derived from the code.",
        all_set_apart: "All of them are set apart below.",
        source_files: "Source files",
        primary_actor: "Primary actor:",
        triggered_by: "Triggered by:",
        what_happens: "What happens",
        technical_details: |steps| {
            format!(
                "Technical details ({steps} step{})",
                if steps == 1 { "" } else { "s" }
            )
        },
        steps: "Steps",
        diagram: "Diagram",
        human: "human",
        system: "system",
        code: "Code:",
        test: "Test:",
        documentation: "Documentation:",
        section: "section",
        validation: "Validation",
        permission: "Permission",
        state_transition: "State transition",
        limit: "Limit",
        computation: "Computation",
        business_rules: "Business rules",
        entities: "Entities:",
        rule_use_cases: "Use cases:",
        rules_index_head: |count| {
            format!(
                "# Business rules\n\n> What is allowed, refused, limited and in which states, by \
                 entity: {count} rule(s) stated from the code, tests and documentation. A rule \
                 backed by code alone (⚠️) is a hypothesis to check against the code.\n\n"
            )
        },
        other_rules: "Other rules",
        existing_documentation: "Existing documentation",
        existing_doc: "Existing doc:",
        states_otherwise: "states otherwise:",
        agent_index_head: "# Functional documentation\n\n\
             > What the application does, in business terms: domains, features and use cases derived \
             from its code and history by RetroDoc. Every page carries a confidence score; below 50% \
             (⚠️) the page is not backed by the code and must be checked against it.\n\n\
             ## How to read it\n\n\
             - Go from a domain to its features, then to their use cases: each page links to the next \
             level and lists the source files it rests on.\n\
             - The pages are generated and date from the last `retrodoc generate` run (see the \
             [run metadata](../_retrodoc/run-metadata.json)): check the cited files if the code has \
             changed since.\n\
             - An agent that can run commands gets the same documentation, with a warning when the \
             cited code changed, from the MCP server: `retrodoc mcp --path <repo>`.\n\n\
             ## Domains\n\n",
        no_domain: "No domain documented yet.",
        agent_index_rules: |link, count| {
            format!(
                "\n## Business rules\n\n- [Business rules by entity]({link}): {count} rule(s) on \
                 what is allowed, refused, limited and in which states, each with its evidence; \
                 they are also on the pages of their feature, use cases and domain\n"
            )
        },
        agent_index_optional: "\n## Optional\n\n\
             - [Documentation debt report](../_retrodoc/coverage-report.md): the sections with a low \
             confidence and the code that no domain covers\n",
        human_edit: "Human edit",
        edited_fields: ["name", "description", "narrative"],
        edited_by: "by",
    };

    pub const FRENCH: Self = Self {
        confidence: "Confiance :",
        business_language: "Langage métier :",
        not_scored: "non évalué",
        technical_heading: "Administration et technique",
        technical_note: "Outils de back-office, et comportements qui se lisent comme du code ou que \
             le code étaye mal.",
        features: "Fonctionnalités",
        domain_overview: "Vue du domaine",
        use_cases: "Cas d'usage",
        no_use_case: "Aucun n'a pu être tiré du code.",
        all_set_apart: "Tous sont mis à part ci-dessous.",
        source_files: "Fichiers sources",
        primary_actor: "Acteur principal :",
        triggered_by: "Déclenché par :",
        what_happens: "Ce qui se passe",
        technical_details: |steps| {
            format!(
                "Détails techniques ({steps} étape{})",
                if steps > 1 { "s" } else { "" }
            )
        },
        steps: "Étapes",
        diagram: "Diagramme",
        human: "humain",
        system: "système",
        code: "Code :",
        test: "Test :",
        documentation: "Documentation :",
        section: "section",
        validation: "Validation",
        permission: "Permission",
        state_transition: "Transition d'état",
        limit: "Limite",
        computation: "Calcul",
        business_rules: "Règles métier",
        entities: "Entités :",
        rule_use_cases: "Cas d'usage :",
        rules_index_head: |count| {
            format!(
                "# Règles métier\n\n> Ce qui est permis, refusé, limité et dans quels états, par \
                 entité : {count} règle(s) énoncée(s) depuis le code, les tests et la \
                 documentation. Une règle étayée par le code seul (⚠️) est une hypothèse à \
                 vérifier contre le code.\n\n"
            )
        },
        other_rules: "Autres règles",
        existing_documentation: "Documentation existante",
        existing_doc: "Doc existante :",
        states_otherwise: "affirme autre chose :",
        agent_index_head: "# Documentation fonctionnelle\n\n\
             > Ce que fait l'application, en termes métier : domaines, fonctionnalités et cas \
             d'usage tirés de son code et de son historique par RetroDoc. Chaque page porte un score \
             de confiance ; sous 50% (⚠️) la page n'est pas étayée par le code et doit être vérifiée \
             contre lui.\n\n\
             ## Comment la lire\n\n\
             - Aller d'un domaine à ses fonctionnalités, puis à leurs cas d'usage : chaque page mène \
             au niveau suivant et liste les fichiers sources sur lesquels elle repose.\n\
             - Les pages sont générées et datent du dernier `retrodoc generate` (voir les \
             [métadonnées du run](../_retrodoc/run-metadata.json)) : vérifier les fichiers cités si \
             le code a changé depuis.\n\
             - Un agent qui peut lancer des commandes obtient la même documentation, avec un \
             avertissement quand le code cité a changé, depuis le serveur MCP : \
             `retrodoc mcp --path <repo>`.\n\n\
             ## Domaines\n\n",
        no_domain: "Aucun domaine documenté pour l'instant.",
        agent_index_rules: |link, count| {
            format!(
                "\n## Règles métier\n\n- [Règles métier par entité]({link}) : {count} règle(s) sur \
                 ce qui est permis, refusé, limité et dans quels états, chacune avec sa preuve ; \
                 elles sont aussi sur les pages de leur fonctionnalité, de leurs cas d'usage et de \
                 leur domaine\n"
            )
        },
        agent_index_optional: "\n## Optionnel\n\n\
             - [Rapport de dette documentaire](../_retrodoc/coverage-report.md) : les sections de \
             faible confiance et le code qu'aucun domaine ne couvre\n",
        human_edit: "Correction humaine",
        edited_fields: ["nom", "description", "récit"],
        edited_by: "par",
    };

    /// The table of the language of the two-letter `tag` (`OutputConfig::language_tag`): English
    /// for no tag or a language with no table.
    #[must_use]
    pub fn for_tag(tag: Option<&str>) -> &'static Self {
        match tag {
            Some("fr") => &Self::FRENCH,
            _ => &Self::ENGLISH,
        }
    }

    /// The name of `field` in the mark of an edit: `narrative`.
    #[must_use]
    pub fn edited_field(&self, field: EditedField) -> &'static str {
        let [name, description, narrative] = self.edited_fields;
        match field {
            EditedField::Name => name,
            EditedField::Description => description,
            EditedField::Narrative => narrative,
        }
    }

    /// The name of a kind of rule in the docs: `State transition`.
    #[must_use]
    pub fn kind(&self, kind: RuleKind) -> &'static str {
        match kind {
            RuleKind::Validation => self.validation,
            RuleKind::Permission => self.permission,
            RuleKind::StateTransition => self.state_transition,
            RuleKind::Limit => self.limit,
            RuleKind::Computation => self.computation,
        }
    }
}

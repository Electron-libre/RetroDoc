//! The fixed text of the pages, one table per language (`specs/web-ui.md`, WEB-77): the labels,
//! headings and messages around the docs. The words the pages share with the Markdown docs come
//! from the table of `retrodoc-render` ([`WebTexts::doc`]). A language with no table keeps English.

use std::fmt::Write as _;

use retrodoc_core::model::SetApartReason;
use retrodoc_llm::CallTotals;
use retrodoc_mcp::debt::{DebtKind, ItemKind, Priority};
use retrodoc_mcp::reviews::Status;
use retrodoc_mcp::source::{SourceError, MAX_FILE_BYTES};
use retrodoc_mcp::{EntryKind, SourceKind, Warning};
use retrodoc_render::Texts;

use crate::code::CiterKind;
use crate::view::{EvidenceKind, Group, ReferencePage};

/// The fixed text of the pages in one language.
#[derive(Debug)]
pub struct WebTexts {
    /// The words shared with the Markdown docs.
    pub doc: &'static Texts,
    pub read: &'static str,
    pub look_up: &'static str,
    pub audit: &'static str,
    pub search_docs: &'static str,
    /// The indicator of a form that writes (review, edit) while it is sent.
    pub saving: &'static str,
    /// The indicator of the ask form while its question is sent.
    pub sending: &'static str,
    /// The indicator of the search and the filters while they load.
    pub loading: &'static str,
    pub tree_label: &'static str,
    pub no_domain_yet: &'static str,
    pub home: &'static str,
    pub who_uses_it: &'static str,
    pub start_here: &'static str,
    pub start_intro: &'static str,
    pub domains: &'static str,
    pub no_domain_generated: &'static str,
    pub stale: &'static str,
    pub stale_hint: &'static str,
    /// The unit of a count of features, singular then plural.
    pub feature_unit: (&'static str, &'static str),
    pub use_case_unit: (&'static str, &'static str),
    /// The pages of reference, in the order a first-time reader is pointed to them.
    pub reference_pages: [ReferencePage; 8],
    pub search: &'static str,
    pub search_intro: &'static str,
    pub results_for: &'static str,
    /// A colon after a label, as the language writes it.
    pub colon: &'static str,
    /// Whether a count of 0 takes the singular, as in French.
    pub zero_is_singular: bool,
    pub search_docs_heading: &'static str,
    /// The kind of a hit of the search panel: domain, feature, use case, rule, concept, collected doc.
    pub hit_kinds: [&'static str; 6],
    /// The link of the search panel to the search page, before the query.
    pub all_results: &'static str,
    pub not_documented: &'static str,
    /// `nothing among`, before the number of entries.
    pub nothing_among: &'static str,
    /// `entries matches`, between the number of entries and the query.
    pub entries_match: &'static str,
    pub no_entry_has: &'static str,
    pub one_left_out: &'static str,
    /// After the number of entries that share only part of the query.
    pub many_left_out: &'static str,
    pub try_other_words: &'static str,
    /// The search panel before a first `generate`.
    pub no_run_results: &'static str,
    pub no_run_title: &'static str,
    pub no_run_heading: &'static str,
    /// Before `retrodoc generate`, then between it and `retrodoc ui`, then after it.
    pub no_run_text: (&'static str, &'static str, &'static str),
    pub not_found: &'static str,
    pub page_missing: &'static str,
    pub back_to_domains: &'static str,
    pub map_of_this_domain: &'static str,
    /// After the link to the map of a domain.
    pub map_of_this_domain_tail: &'static str,
    pub back_office: &'static str,
    pub entry_points: &'static str,
    pub unbacked: &'static str,
    pub unbacked_hint: &'static str,
    pub diagram_source: &'static str,
    pub sequence_label: &'static str,
    pub previous: &'static str,
    pub next: &'static str,
    pub breadcrumb: &'static str,
    /// The badge of a doc section that contradicts an item.
    pub states_otherwise: &'static str,
    pub changed_since_the_run: &'static str,
    pub deleted: &'static str,
    pub modified: &'static str,
    pub all: &'static str,
    pub filter: &'static str,
    /// The unit of a count of active filters, singular then plural.
    pub active_filter_unit: (&'static str, &'static str),
    pub reset_filters: &'static str,
    pub kind: &'static str,
    pub domain: &'static str,
    pub feature: &'static str,
    pub use_case: &'static str,
    pub rule: &'static str,
    pub rule_feature: &'static str,
    pub not_in_the_run: &'static str,
    pub no_rule_in_run: &'static str,
    pub no_rule_matches: &'static str,
    /// The list `htmx` swaps under the form of the rules before a first `generate`.
    pub no_run_rules: &'static str,
    pub no_concept: &'static str,
    pub glossary_intro: &'static str,
    pub no_use_case_uses_it: &'static str,
    pub used_by: &'static str,
    pub no_actor: &'static str,
    pub actors_intro: &'static str,
    pub people: &'static str,
    pub systems: &'static str,
    pub actor: &'static str,
    pub permission_rules: &'static str,
    pub starts: &'static str,
    pub map_intro: &'static str,
    /// Before the domain a domain shares concepts with.
    pub with: &'static str,
    pub map_of: &'static str,
    pub no_feature_in_domain: &'static str,
    /// Before the link to the domain, then after it.
    pub domain_map_intro: (&'static str, &'static str),
    pub actors_label: &'static str,
    pub concepts_label: &'static str,
    /// After the counts of the files cited and of all the files.
    pub files_cited: &'static str,
    pub files_hint: &'static str,
    pub dir_cited: &'static str,
    pub cited_by_count: &'static str,
    pub not_cited: &'static str,
    pub cited_by: &'static str,
    pub code_heading: &'static str,
    pub file_empty: &'static str,
    pub showing: &'static str,
    /// Before the length of a line cut.
    pub line_cut: &'static str,
    pub characters: &'static str,
    pub next_lines: &'static str,
    pub history: &'static str,
    pub no_commit: &'static str,
    pub renamed_from: &'static str,
    pub no_history: &'static str,
    /// Above a collected doc.
    pub existing_doc: &'static str,
    pub debt: DebtTexts,
    pub audit_page: AuditTexts,
    pub history_page: HistoryTexts,
    pub review: ReviewTexts,
    pub edit: EditTexts,
    pub ask: AskTexts,
    /// Why a form was refused, shown beside its fields.
    pub errors: ErrorTexts,
    /// The lines a page of a file shows: from, to, of how many.
    pub shown: fn(usize, usize, usize) -> String,
    /// Why the lines asked are not shown: the number of lines of the file, the first line asked.
    pub past_end: fn(usize, usize) -> String,
    /// Why a cited file can't be shown.
    pub refusal: fn(&SourceError) -> String,
}

/// The fixed text of the ask page (`specs/web-ask.md`).
#[derive(Debug)]
pub struct AskTexts {
    /// The link of the header and the title of the page.
    pub link: &'static str,
    pub hint: &'static str,
    pub intro: &'static str,
    pub question: &'static str,
    pub send: &'static str,
    /// Which keys send the question and which break its line (WAK-41).
    pub keys: &'static str,
    /// The label of the list of suggested questions (WAK-43).
    pub suggested: &'static str,
    /// The suggested question on the purpose of the application.
    pub purpose_question: &'static str,
    /// The suggested question on a domain, given its name.
    pub domain_question: fn(&str) -> String,
    /// Shown while a question runs.
    pub waiting: &'static str,
    pub start_over: &'static str,
    pub sources: &'static str,
    pub warnings: &'static str,
    pub no_source: &'static str,
    pub no_answer: &'static str,
    /// Before the error of a question that failed.
    pub failed: &'static str,
    /// The summary of the steps of an exchange: how many tool calls it made.
    pub steps: fn(usize) -> String,
    /// Before the cost of the question, then of the session.
    pub question_cost: &'static str,
    pub session_cost: &'static str,
    /// The LLM calls of a question or a session and their tokens.
    pub cost: fn(&CallTotals) -> String,
    /// The label of a domain, a feature, a use case and a file.
    pub kinds: [&'static str; 4],
    /// A warning of an answer.
    pub warning: fn(&Warning, &AskTexts) -> String,
}

impl AskTexts {
    /// The label of `kind`.
    #[must_use]
    pub fn kind(&self, kind: SourceKind) -> &'static str {
        match kind {
            SourceKind::Domain => self.kinds[0],
            SourceKind::Feature => self.kinds[1],
            SourceKind::UseCase => self.kinds[2],
            SourceKind::File => self.kinds[3],
        }
    }
}

/// The fixed text of the debt page.
#[derive(Debug)]
pub struct DebtTexts {
    pub intro: &'static str,
    /// The name of each kind, in the order of `DebtKind::ALL`.
    pub kinds: [&'static str; 15],
    /// Why a use case is set apart: its back-office feature, its business language, its confidence.
    pub reasons: [&'static str; 3],
    pub set_apart_for: &'static str,
    pub none: &'static str,
    pub none_matches: &'static str,
    pub no_score: &'static str,
    pub no_docs: &'static str,
    /// What a doc item is: a domain, a feature, a use case, a rule.
    pub item_kinds: [&'static str; 4],
    /// `entry point`, `entry points`, `file`, `files`.
    pub counted: [&'static str; 4],
    /// The priority of an item and its parts: `Priority 0.52: weight 0.25, weakness 1.00, 2 commits, 0
    /// entry points`.
    pub priority: fn(f64, f32, f32, String, String) -> String,
    /// `commit`, `commits`.
    pub commits: [&'static str; 2],
    /// The git history could not be read: no item counts a commit.
    pub no_history: &'static str,
    /// Before what a contradicting section states, and before what the docs state.
    pub doc_states: &'static str,
    pub docs_state: &'static str,
    /// The list `htmx` swaps under the form before a first `generate`.
    pub no_run: &'static str,
    /// The file of the reviews could not be read: the debt is listed as if no section was reviewed.
    pub reviews_unreadable: &'static str,
}

/// The fixed text of the audit page.
#[derive(Debug)]
pub struct AuditTexts {
    pub intro: &'static str,
    pub confidence: &'static str,
    pub business_language: &'static str,
    /// Why the run has no confidence score, with the command that scores it.
    pub no_confidence: &'static str,
    pub coverage: &'static str,
    pub placed: &'static str,
    pub cited: &'static str,
    pub no_coverage: &'static str,
    pub stale: &'static str,
    pub stale_sections: &'static str,
    pub cost: &'static str,
    pub finished: &'static str,
    /// The columns of the cost: the pass, its duration, calls, prompt, cached and completion tokens.
    pub cost_columns: [&'static str; 6],
    pub total: &'static str,
    pub lower_bound: &'static str,
    pub no_cost: &'static str,
    pub contexts: &'static str,
    pub contexts_done: &'static str,
    pub pending: &'static str,
    pub no_contexts: &'static str,
    pub reviews: &'static str,
    /// The domains, the features, the use cases reviewed.
    pub reviewed: [&'static str; 3],
    /// The file of the reviews could not be read: the share reviewed is missing.
    pub reviews_unreadable: &'static str,
    pub heatmap: &'static str,
    pub heatmap_intro: &'static str,
    pub fewer: &'static str,
    pub more: &'static str,
    /// Between a count and its total: `2 of 5`.
    pub of: &'static str,
    pub trends: &'static str,
    pub trends_intro: &'static str,
    /// Why there are no trends, with the command that saves a run.
    pub no_trends: &'static str,
    /// The columns of the trends: the run, its commit, the confidence, the business language, the
    /// two shares of the coverage, the debt items.
    pub trend_columns: [&'static str; 7],
    pub trends_debt: &'static str,
    /// After a figure worse than in the run before.
    pub worse: &'static str,
    /// What a chart of the trends shows, for a screen reader.
    pub chart_note: &'static str,
}

/// The fixed text of the history page.
#[derive(Debug)]
pub struct HistoryTexts {
    pub intro: &'static str,
    pub runs: &'static str,
    pub no_commit: &'static str,
    /// Why there is no difference, with the command that saves a run.
    pub fewer: &'static str,
    /// Before the names of the runs asked that are not saved.
    pub unknown: &'static str,
    /// The heads of the columns of the runs: the choice of the "from" run, of the "to" run, the end, the commit.
    pub from: &'static str,
    pub to: &'static str,
    pub ended: &'static str,
    pub commit: &'static str,
    pub swap: &'static str,
    pub show: &'static str,
    pub no_difference: &'static str,
    /// After a section the docs served don't hold.
    pub no_page: &'static str,
    pub added: &'static str,
    pub removed: &'static str,
    pub degraded: &'static str,
    pub set_apart: &'static str,
    pub no_longer_set_apart: &'static str,
    pub confidence: &'static str,
    pub business_language: &'static str,
}

/// Why a form was refused, the fixed reasons; the errors of a file come from the store, in English.
#[derive(Debug)]
pub struct ErrorTexts {
    pub unknown_status: &'static str,
    pub no_section: &'static str,
    pub no_repository: &'static str,
    pub narrative_of_use_case_only: &'static str,
    pub blank_question: &'static str,
    pub question_running: &'static str,
    pub no_docs: &'static str,
}

/// The fixed text of the human edit of a section. The mark of an edit and the names of its fields
/// are the docs' (`Texts::human_edit`, `Texts::edited_fields`), so that the page and the Markdown
/// say the same (SED-38).
#[derive(Debug)]
pub struct EditTexts {
    /// The summary of the text the last run generated for the fields of an edit.
    pub generated: &'static str,
    pub outdated: OutdatedTexts,
    /// The mark of an edited section in a list.
    pub badge: &'static str,
    /// The summary of the folded edit form.
    pub form_heading: &'static str,
    /// The labels of the fields of the form, in the order of `Field::ALL`.
    pub labels: [&'static str; 3],
    /// Under the fields: what an empty one or one left as generated does.
    pub form_hint: &'static str,
    pub save: &'static str,
    /// The notice of a save.
    pub saved: &'static str,
    /// The button that removes the edit.
    pub revert: &'static str,
}

/// Why a review or an edit is outdated, in words.
#[derive(Debug)]
pub struct OutdatedTexts {
    /// Before the reasons.
    pub heading: &'static str,
    pub rewritten: &'static str,
    /// Before the files changed since.
    pub files_changed: &'static str,
}

/// The fixed text of the review of a section.
#[derive(Debug)]
pub struct ReviewTexts {
    pub heading: &'static str,
    pub not_reviewed: &'static str,
    /// Before the author of a review.
    pub by: &'static str,
    /// Validated, wrong, to review.
    pub statuses: [&'static str; 3],
    pub status_label: &'static str,
    /// The choice of the form that removes the review.
    pub no_status: &'static str,
    pub comment: &'static str,
    pub save: &'static str,
    /// The notice of a save.
    pub saved: &'static str,
    pub outdated: OutdatedTexts,
    /// Beside the badge of an outdated review in a list.
    pub outdated_badge: &'static str,
    /// Before the number of outdated reviews on the audit page.
    pub outdated_count: &'static str,
}

impl ReviewTexts {
    /// The label of `status`.
    #[must_use]
    pub fn status(&self, status: Status) -> &'static str {
        match status {
            Status::Validated => self.statuses[0],
            Status::Wrong => self.statuses[1],
            Status::ToReview => self.statuses[2],
        }
    }
}

impl WebTexts {
    pub const ENGLISH: Self = Self {
        doc: &Texts::ENGLISH,
        read: "Read",
        look_up: "Look up",
        audit: "Audit",
        search_docs: "Search the docs",
        saving: "Saving…",
        sending: "Sending…",
        loading: "Loading…",
        tree_label: "Domains, features and use cases",
        no_domain_yet: "No domain yet.",
        home: "Home",
        who_uses_it: "Who uses it",
        start_here: "Start here",
        start_intro: "Pick a domain below to read what the application does, or look things up \
            across the docs:",
        domains: "Domains",
        no_domain_generated: "The last run generated no domain.",
        stale: "stale",
        stale_hint: "A file it cites changed since the run",
        feature_unit: ("feature", "features"),
        use_case_unit: ("use case", "use cases"),
        reference_pages: [
            ReferencePage {
                href: "/map",
                name: "Map",
                hint:
                    "The domains drawn as a graph, linked by the concepts they share; each opens \
                    its features.",
                group: Group::Read,
                unit: Some(("domain", "domains")),
            },
            ReferencePage {
                href: "/actors",
                name: "Actors",
                hint: "Who uses the application, and which use cases each one starts or takes \
                    part in.",
                group: Group::Read,
                unit: Some(("actor", "actors")),
            },
            ReferencePage {
                href: "/glossary",
                name: "Glossary",
                hint: "The business concepts of the application, defined, with the use cases \
                    that use them.",
                group: Group::LookUp,
                unit: Some(("concept", "concepts")),
            },
            ReferencePage {
                href: "/rules",
                name: "Rules",
                hint: "The business rules the code enforces, filtered by kind, domain or feature.",
                group: Group::LookUp,
                unit: Some(("rule", "rules")),
            },
            ReferencePage {
                href: "/files",
                name: "Files",
                hint: "The source files, with the docs that cite each one and those no doc covers.",
                group: Group::LookUp,
                unit: Some(("cited file", "cited files")),
            },
            ReferencePage {
                href: "/debt",
                name: "Debt",
                hint: "The sections of the docs that need a human, filtered by kind or domain.",
                group: Group::Audit,
                unit: Some(("item", "items")),
            },
            ReferencePage {
                href: "/audit",
                name: "Dashboard",
                hint: "The figures of the last run and where the debt of the docs concentrates, \
                    domain by kind.",
                group: Group::Audit,
                unit: None,
            },
            ReferencePage {
                href: "/history",
                name: "History",
                hint: "The saved runs, and the sections one run added, removed or made worse \
                    compared with another.",
                group: Group::Audit,
                unit: None,
            },
        ],
        search: "Search",
        search_intro: "Search the domains, features, use cases, business rules, glossary and \
            existing docs, in the application's own words.",
        results_for: "Results for",
        colon: ":",
        zero_is_singular: false,
        search_docs_heading: "Existing docs",
        hit_kinds: [
            "Domain",
            "Feature",
            "Use case",
            "Business rule",
            "Concept",
            "Existing doc",
        ],
        all_results: "All results for",
        not_documented: "Not documented:",
        nothing_among: "nothing among",
        entries_match: "entries matches",
        no_entry_has: "No entry has",
        one_left_out: "1 entry shares only part of the query.",
        many_left_out: "entries share only part of the query.",
        try_other_words: "Try other words.",
        no_run_results: "<p class=\"empty\">Nothing to search yet: run \
            <code>retrodoc generate</code>, then restart <code>retrodoc ui</code>.</p>",
        no_run_title: "No run yet",
        no_run_heading: "No docs generated yet",
        no_run_text: ("This repository has no run yet. Run", ", then restart", "."),
        not_found: "Not found",
        page_missing: "This page does not exist.",
        back_to_domains: "Back to the domains",
        map_of_this_domain: "Map of this domain",
        map_of_this_domain_tail: ": its features, who uses them and the concepts they use.",
        back_office: "back-office",
        entry_points: "Entry points",
        unbacked: "unbacked",
        unbacked_hint: "The confidence pass judged it unsupported by the cited code",
        diagram_source: "Source of the diagram",
        sequence_label: "Use cases of this feature",
        previous: "Previous:",
        next: "Next:",
        breadcrumb: "Breadcrumb",
        states_otherwise: "states otherwise",
        changed_since_the_run: "changed since the run:",
        deleted: "deleted",
        modified: "modified",
        all: "All",
        filter: "Filter",
        active_filter_unit: ("active filter", "active filters"),
        reset_filters: "Reset filters",
        kind: "Kind",
        domain: "Domain",
        feature: "Feature",
        use_case: "Use case",
        rule: "Rule",
        rule_feature: "Feature:",
        not_in_the_run: "not in the run",
        no_rule_in_run: "No business rule was found in the code of the last run.",
        no_rule_matches: "No rule matches.",
        no_run_rules: "<section id=\"rule-list\"><p class=\"empty\">No rule yet: run <code>retrodoc generate</code>, then restart <code>retrodoc ui</code>.</p></section>",
        no_concept: "No concept was found in the code of the last run.",
        glossary_intro: "The business concepts read from the code, with the use cases that use them.",
        no_use_case_uses_it: "No use case uses it.",
        used_by: "Used by:",
        no_actor: "No actor was found in the use cases of the last run.",
        actors_intro: "Who starts or takes part in the use cases of each feature, as the use cases name them.",
        people: "People",
        systems: "Systems",
        actor: "Actor",
        permission_rules: "Permission rules",
        starts: "starts",
        map_intro: "The domains of the application; two are linked when their use cases use the same concepts. Open a domain to see its features, their actors and their concepts.",
        with: "with",
        map_of: "Map of",
        no_feature_in_domain: "This domain has no feature.",
        domain_map_intro: ("The features of", ", the people who start their use cases (solid arrow) or take part in them (dotted arrow), and the concepts they use."),
        actors_label: "Actors:",
        concepts_label: "Concepts:",
        files_cited: "files cited by the docs.",
        files_hint: "A file opens its code, with what cites it.",
        dir_cited: "cited",
        cited_by_count: "cited by",
        not_cited: "not cited",
        cited_by: "Cited by",
        code_heading: "Code",
        file_empty: "The file is empty.",
        showing: "Showing",
        line_cut: "line cut,",
        characters: "characters",
        next_lines: "Next lines",
        history: "History",
        no_commit: "No commit touches this file (not committed yet?).",
        renamed_from: "renamed from",
        no_history: "The history is not available: this repository has no readable git history.",
        existing_doc: "Existing doc",
        shown: |start, end, total| format!("lines {start}-{end} of {total}"),
        past_end: |total, line| format!("The file has {total} lines: line {line} is past the end."),
        debt: DebtTexts {
            intro: "The sections of the docs that need a human: to reread, to fix in the code or \
                to document by hand; then the items the project's own docs contradict or leave out, \
                and the code no doc covers, by folder.",
            kinds: [
                "Low confidence",
                "Not scored",
                "Unbacked narrative",
                "Technical language",
                "Set apart",
                "Stale",
                "Contradicted",
                "Undocumented",
                "Marked wrong",
                "To review",
                "Outdated edit",
                "Entry point without a feature",
                "No domain",
                "Reached by no behaviour",
                "Not analysed",
            ],
            reasons: ["back-office feature", "business language", "confidence"],
            set_apart_for: "Set apart for:",
            none: "No section of the docs needs a human.",
            none_matches: "No item matches.",
            no_score: "The last run has no confidence score: its use cases are not listed as not \
                scored. Run <code>retrodoc generate</code> again to score them.",
            no_docs: "No project doc to compare the items with: none is listed as contradicted or \
                undocumented. Name the docs in <code>existing_docs_paths</code> of <code>retrodoc.toml</code>.",
            item_kinds: ["domain", "feature", "use case", "rule"],
            counted: ["entry point", "entry points", "file", "files"],
            priority: |value, weight, weakness, commits, entry_points| {
                format!(
                    "Priority {value:.2}: weight {weight:.2}, weakness {weakness:.2}, {commits}, \
                     {entry_points}"
                )
            },
            commits: ["commit", "commits"],
            no_history: "The git history could not be read: every item counts 0 commits.",
            doc_states: "states",
            docs_state: "the generated docs state",
            no_run: "<section id=\"debt-list\"><p class=\"empty\">No debt yet: run <code>retrodoc generate</code>, then restart <code>retrodoc ui</code>.</p></section>",
            reviews_unreadable: "The reviews could not be read: the debt is listed as if no section \
                was reviewed.",
        },
        audit_page: AuditTexts {
            intro: "The figures of the last run, and where the debt of the docs concentrates: \
                each cell opens the debt of its domain and kind.",
            confidence: "Overall confidence",
            business_language: "Business language",
            no_confidence: "Missing: the last run has no confidence score. Run \
                <code>retrodoc generate</code> to score its use cases.",
            coverage: "Entry points",
            placed: "held by a feature",
            cited: "cited by a use case",
            no_coverage: "Missing: no coverage of the entry points saved. A \
                <code>retrodoc generate</code> with <code>domains_from = \"contexts\"</code> or \
                <code>\"surface\"</code> saves it.",
            stale: "Stale sections",
            stale_sections: "features and use cases cite a file changed since the run",
            cost: "Cost of the last generate",
            finished: "Finished on",
            cost_columns: [
                "Pass",
                "Duration",
                "Calls",
                "Prompt tokens",
                "Cached tokens",
                "Completion tokens",
            ],
            total: "Total",
            lower_bound: "lower bound: some calls reported no tokens",
            no_cost: "Missing: no <code>retrodoc generate</code> in the usage history of this \
                repository.",
            contexts: "Bounded contexts",
            contexts_done: "contexts done",
            pending: "Pending:",
            reviews: "Human review",
            reviewed: ["Domains reviewed", "Features reviewed", "Use cases reviewed"],
            reviews_unreadable: "Missing: the reviews could not be read. Fix or remove \
                <code>.retrodoc/reviews.json</code>; the heatmap counts no review meanwhile.",
            no_contexts: "Missing: no map of the bounded contexts. A \
                <code>retrodoc generate</code> with <code>domains_from = \"contexts\"</code> \
                maps them.",
            heatmap: "Where the debt is",
            heatmap_intro: "The debt items of each domain by kind; the brighter the gold of a cell, the \
                more items.",
            fewer: "fewer",
            more: "more",
            of: "of",
            trends: "Trends",
            trends_intro: "The figures of each saved run of <code>retrodoc generate</code>, newest \
                first; a figure worse than in the run before is marked ▼.",
            no_trends: "The trends need two runs: each <code>retrodoc generate</code> that changes \
                the docs saves one.",
            trend_columns: [
                "Run",
                "Commit",
                "Confidence",
                "Business language",
                "Placed in a feature",
                "Cited by a use case",
                "Debt items",
            ],
            trends_debt: "Debt items by kind",
            worse: "worse",
            chart_note: "over the saved runs, oldest left; the table below holds the figures",
        },
        history_page: HistoryTexts {
            intro: "The runs of <code>retrodoc generate</code> that changed the docs, and what \
                one run changed in the sections of another.",
            runs: "Saved runs",
            no_commit: "no commit",
            fewer: "The difference needs two runs: each <code>retrodoc generate</code> that \
                changes the docs saves one.",
            unknown: "No saved run is named:",
            from: "From",
            to: "To",
            ended: "Ended",
            commit: "Commit",
            swap: "Swap the runs",
            show: "Show",
            no_difference: "The sections of these two runs do not differ.",
            no_page: "no page",
            added: "Added",
            removed: "Removed",
            degraded: "Degraded",
            set_apart: "Newly set apart",
            no_longer_set_apart: "No longer set apart",
            confidence: "Confidence",
            business_language: "Business language",
        },
        review: ReviewTexts {
            heading: "Review",
            not_reviewed: "Not reviewed",
            by: "by",
            statuses: ["Validated", "Wrong", "To review"],
            status_label: "Status",
            no_status: "No review",
            comment: "Comment",
            save: "Save",
            saved: "Review saved.",
            outdated: OutdatedTexts {
                heading: "Outdated review",
                rewritten: "section rewritten since the review",
                files_changed: "files changed since the review:",
            },
            outdated_badge: "outdated",
            outdated_count: "Outdated",
        },
        errors: ErrorTexts {
            unknown_status: "Unknown review status.",
            no_section: "The last run has no such section.",
            no_repository: "The docs carry no repository to save in.",
            narrative_of_use_case_only: "Only a use case has a narrative.",
            blank_question: "The question is empty.",
            question_running: "A question of this conversation is running.",
            no_docs: "No docs to ask: run `retrodoc generate`.",
        },
        edit: EditTexts {
            generated: "Generated by the last run",
            outdated: OutdatedTexts {
                heading: "Outdated edit",
                rewritten: "section rewritten since the edit",
                files_changed: "files changed since the edit:",
            },
            badge: "edited",
            form_heading: "Edit this section",
            labels: ["Name", "Description", "Narrative"],
            form_hint: "An empty field, or one left as generated, keeps the generated text.",
            save: "Save the edit",
            saved: "Edit saved.",
            revert: "Back to the generated text",
        },
        ask: AskTexts {
            link: "Ask",
            hint: "Ask a question about the application; the answer reads the docs with an LLM",
            intro: "Ask a question about the application: an LLM answers from the docs, naming the pages it rests on. Each question is sent with the earlier ones of this conversation.",
            question: "Your question",
            send: "Ask",
            keys: "Enter sends, Shift+Enter breaks the line.",
            suggested: "Suggested questions",
            purpose_question: "What is this application for?",
            domain_question: |name| format!("What does the domain {name} cover?"),
            waiting: "Reading the docs to answer…",
            start_over: "Start over",
            sources: "Sources",
            warnings: "Warnings",
            no_source: retrodoc_agent::NO_SOURCE,
            no_answer: retrodoc_agent::NO_ANSWER,
            failed: "The question could not be answered:",
            steps: |n| format!("{n} step{}", if n == 1 { "" } else { "s" }),
            question_cost: "This question:",
            session_cost: "This conversation:",
            cost: english_cost,
            kinds: ["domain", "feature", "use case", "file"],
            warning: english_warning,
        },
        refusal: SourceError::to_string,
    };

    pub const FRENCH: Self = Self {
        doc: &Texts::FRENCH,
        read: "Lire",
        look_up: "Consulter",
        audit: "Auditer",
        search_docs: "Rechercher dans la doc",
        saving: "Enregistrement…",
        sending: "Envoi…",
        loading: "Chargement…",
        tree_label: "Domaines, fonctionnalités et cas d'usage",
        no_domain_yet: "Aucun domaine pour l'instant.",
        home: "Accueil",
        who_uses_it: "Qui l'utilise",
        start_here: "Commencer ici",
        start_intro: "Choisissez un domaine ci-dessous pour lire ce que fait l'application, ou \
            cherchez dans toute la doc :",
        domains: "Domaines",
        no_domain_generated: "Le dernier run n'a produit aucun domaine.",
        stale: "périmé",
        stale_hint: "Un fichier qu'il cite a changé depuis le run",
        feature_unit: ("fonctionnalité", "fonctionnalités"),
        use_case_unit: ("cas d'usage", "cas d'usage"),
        reference_pages: [
            ReferencePage {
                href: "/map",
                name: "Carte",
                hint:
                    "Les domaines dessinés en graphe, reliés par les concepts qu'ils partagent ; \
                    chacun ouvre ses fonctionnalités.",
                group: Group::Read,
                unit: Some(("domaine", "domaines")),
            },
            ReferencePage {
                href: "/actors",
                name: "Acteurs",
                hint: "Qui utilise l'application, et quels cas d'usage chacun lance ou auxquels \
                    il prend part.",
                group: Group::Read,
                unit: Some(("acteur", "acteurs")),
            },
            ReferencePage {
                href: "/glossary",
                name: "Glossaire",
                hint: "Les concepts métier de l'application, définis, avec les cas d'usage qui \
                    les utilisent.",
                group: Group::LookUp,
                unit: Some(("concept", "concepts")),
            },
            ReferencePage {
                href: "/rules",
                name: "Règles",
                hint: "Les règles métier que le code applique, filtrées par type, domaine ou \
                    fonctionnalité.",
                group: Group::LookUp,
                unit: Some(("règle", "règles")),
            },
            ReferencePage {
                href: "/files",
                name: "Fichiers",
                hint: "Les fichiers sources, avec les docs qui citent chacun et ceux qu'aucune \
                    doc ne couvre.",
                group: Group::LookUp,
                unit: Some(("fichier cité", "fichiers cités")),
            },
            ReferencePage {
                href: "/debt",
                name: "Dette",
                hint: "Les sections de la doc qui demandent un humain, filtrées par type ou domaine.",
                group: Group::Audit,
                unit: Some(("élément", "éléments")),
            },
            ReferencePage {
                href: "/audit",
                name: "Tableau de bord",
                hint: "Les chiffres du dernier run et où se concentre la dette de la doc, domaine \
                    par type.",
                group: Group::Audit,
                unit: None,
            },
            ReferencePage {
                href: "/history",
                name: "Historique",
                hint: "Les runs enregistrés, et les sections qu'un run a ajoutées, retirées ou \
                    dégradées par rapport à un autre.",
                group: Group::Audit,
                unit: None,
            },
        ],
        search: "Recherche",
        search_intro: "Cherchez dans les domaines, fonctionnalités, cas d'usage, règles métier, \
            le glossaire et les docs existantes, avec les mots de l'application.",
        results_for: "Résultats pour",
        colon: " :",
        zero_is_singular: true,
        search_docs_heading: "Docs existantes",
        hit_kinds: [
            "Domaine",
            "Fonctionnalité",
            "Cas d'usage",
            "Règle métier",
            "Concept",
            "Doc existante",
        ],
        all_results: "Tous les résultats pour",
        not_documented: "Non documenté :",
        nothing_among: "rien parmi",
        entries_match: "entrées ne correspond à",
        no_entry_has: "Aucune entrée ne contient",
        one_left_out: "1 entrée ne partage qu'une partie de la requête.",
        many_left_out: "entrées ne partagent qu'une partie de la requête.",
        try_other_words: "Essayez d'autres mots.",
        no_run_results: "<p class=\"empty\">Rien à chercher pour l'instant : lancez \
            <code>retrodoc generate</code>, puis relancez <code>retrodoc ui</code>.</p>",
        no_run_title: "Pas encore de run",
        no_run_heading: "Aucune doc générée pour l'instant",
        no_run_text: (
            "Ce dépôt n'a pas encore de run. Lancez",
            ", puis relancez",
            ".",
        ),
        not_found: "Page introuvable",
        page_missing: "Cette page n'existe pas.",
        back_to_domains: "Retour aux domaines",
        map_of_this_domain: "Carte de ce domaine",
        map_of_this_domain_tail: " : ses fonctionnalités, qui les utilise et les concepts qu'elles utilisent.",
        back_office: "back-office",
        entry_points: "Points d'entrée",
        unbacked: "non étayé",
        unbacked_hint: "La passe de confiance l'a jugé non étayé par le code cité",
        diagram_source: "Source du diagramme",
        sequence_label: "Cas d'usage de cette fonctionnalité",
        previous: "Précédent :",
        next: "Suivant :",
        breadcrumb: "Fil d'Ariane",
        states_otherwise: "affirme autre chose",
        changed_since_the_run: "modifié depuis le run :",
        deleted: "supprimé",
        modified: "modifié",
        all: "Tous",
        filter: "Filtrer",
        active_filter_unit: ("filtre actif", "filtres actifs"),
        reset_filters: "Réinitialiser les filtres",
        kind: "Type",
        domain: "Domaine",
        feature: "Fonctionnalité",
        use_case: "Cas d'usage",
        rule: "Règle",
        rule_feature: "Fonctionnalité :",
        not_in_the_run: "absente du run",
        no_rule_in_run: "Aucune règle métier n'a été trouvée dans le code du dernier run.",
        no_rule_matches: "Aucune règle ne correspond.",
        no_run_rules: "<section id=\"rule-list\"><p class=\"empty\">Pas encore de règle : lancez <code>retrodoc generate</code>, puis relancez <code>retrodoc ui</code>.</p></section>",
        no_concept: "Aucun concept n'a été trouvé dans le code du dernier run.",
        glossary_intro: "Les concepts métier lus dans le code, avec les cas d'usage qui les utilisent.",
        no_use_case_uses_it: "Aucun cas d'usage ne l'utilise.",
        used_by: "Utilisé par :",
        no_actor: "Aucun acteur n'a été trouvé dans les cas d'usage du dernier run.",
        actors_intro: "Qui lance les cas d'usage de chaque fonctionnalité ou y prend part, tels que les cas d'usage les nomment.",
        people: "Personnes",
        systems: "Systèmes",
        actor: "Acteur",
        permission_rules: "Règles de permission",
        starts: "lance",
        map_intro: "Les domaines de l'application ; deux sont reliés quand leurs cas d'usage utilisent les mêmes concepts. Ouvrez un domaine pour voir ses fonctionnalités, leurs acteurs et leurs concepts.",
        with: "avec",
        map_of: "Carte de",
        no_feature_in_domain: "Ce domaine n'a aucune fonctionnalité.",
        domain_map_intro: ("Les fonctionnalités de", ", les personnes qui lancent leurs cas d'usage (flèche pleine) ou y prennent part (flèche pointillée), et les concepts qu'elles utilisent."),
        actors_label: "Acteurs :",
        concepts_label: "Concepts :",
        files_cited: "fichiers cités par la doc.",
        files_hint: "Un fichier ouvre son code, avec ce qui le cite.",
        dir_cited: "cités",
        cited_by_count: "cité par",
        not_cited: "non cité",
        cited_by: "Cité par",
        code_heading: "Code",
        file_empty: "Le fichier est vide.",
        showing: "Affichage des",
        line_cut: "ligne coupée,",
        characters: "caractères",
        next_lines: "Lignes suivantes",
        history: "Historique",
        no_commit: "Aucun commit ne touche ce fichier (pas encore commité ?).",
        renamed_from: "renommé depuis",
        no_history: "L'historique n'est pas disponible : ce dépôt n'a pas d'historique git lisible.",
        existing_doc: "Doc existante",
        shown: |start, end, total| format!("lignes {start}-{end} sur {total}"),
        past_end: |total, line| format!("Le fichier a {total} lignes : la ligne {line} est après la fin."),
        debt: DebtTexts {
            intro: "Les sections de la doc qui demandent un humain : à relire, à corriger dans le \
                code ou à documenter à la main ; puis les éléments que les docs du projet contredisent \
                ou omettent, et le code qu'aucune doc ne couvre, par dossier.",
            kinds: [
                "Confiance faible",
                "Non évalué",
                "Récit non étayé",
                "Langage technique",
                "Mis à part",
                "Périmé",
                "Contredit",
                "Non documenté",
                "Marqué faux",
                "À revoir",
                "Correction périmée",
                "Point d'entrée sans fonctionnalité",
                "Sans domaine",
                "Atteint par aucun comportement",
                "Non analysé",
            ],
            reasons: ["fonctionnalité back-office", "langage métier", "confiance"],
            set_apart_for: "Mis à part pour :",
            none: "Aucune section de la doc n'a besoin d'un humain.",
            none_matches: "Aucun élément ne correspond.",
            no_score: "Le dernier run n'a aucun score de confiance : ses cas d'usage ne sont pas \
                listés comme non évalués. Relancez <code>retrodoc generate</code> pour les évaluer.",
            no_docs: "Aucune doc du projet à laquelle comparer les éléments : aucun n'est listé comme \
                contredit ou non documenté. Nommez les docs dans <code>existing_docs_paths</code> de <code>retrodoc.toml</code>.",
            item_kinds: ["domaine", "fonctionnalité", "cas d'usage", "règle"],
            counted: ["point d'entrée", "points d'entrée", "fichier", "fichiers"],
            priority: |value, weight, weakness, commits, entry_points| {
                format!(
                    "Priorité {value:.2} : poids {weight:.2}, faiblesse {weakness:.2}, {commits}, \
                     {entry_points}"
                )
            },
            commits: ["commit", "commits"],
            no_history: "L'historique git n'a pas pu être lu : chaque élément compte 0 commit.",
            doc_states: "dit",
            docs_state: "la doc générée dit",
            no_run: "<section id=\"debt-list\"><p class=\"empty\">Pas encore de dette : lancez <code>retrodoc generate</code>, puis relancez <code>retrodoc ui</code>.</p></section>",
            reviews_unreadable: "Les revues n'ont pas pu être lues : la dette est listée comme si \
                aucune section n'avait été revue.",
        },
        audit_page: AuditTexts {
            intro: "Les chiffres du dernier run, et où se concentre la dette de la doc : chaque \
                case ouvre la dette de son domaine et de son type.",
            confidence: "Confiance globale",
            business_language: "Langage métier",
            no_confidence: "Manquant : le dernier run n'a aucun score de confiance. Lancez \
                <code>retrodoc generate</code> pour évaluer ses cas d'usage.",
            coverage: "Points d'entrée",
            placed: "tenus par une fonctionnalité",
            cited: "cités par un cas d'usage",
            no_coverage: "Manquant : aucune couverture des points d'entrée enregistrée. Un \
                <code>retrodoc generate</code> avec <code>domains_from = \"contexts\"</code> ou \
                <code>\"surface\"</code> l'enregistre.",
            stale: "Sections périmées",
            stale_sections: "fonctionnalités et cas d'usage citent un fichier modifié depuis le run",
            cost: "Coût du dernier generate",
            finished: "Terminé le",
            cost_columns: [
                "Passe",
                "Durée",
                "Appels",
                "Tokens du prompt",
                "Tokens en cache",
                "Tokens de réponse",
            ],
            total: "Total",
            lower_bound: "minimum : des appels n'ont rapporté aucun token",
            no_cost: "Manquant : aucun <code>retrodoc generate</code> dans l'historique d'usage \
                de ce dépôt.",
            contexts: "Contextes bornés",
            contexts_done: "contextes terminés",
            pending: "En attente :",
            reviews: "Revue humaine",
            reviewed: ["Domaines revus", "Fonctionnalités revues", "Cas d'usage revus"],
            reviews_unreadable: "Manquant : les revues n'ont pas pu être lues. Corrigez ou supprimez \
                <code>.retrodoc/reviews.json</code> ; d'ici là, la carte de chaleur ne compte aucune revue.",
            no_contexts: "Manquant : aucune carte des contextes bornés. Un \
                <code>retrodoc generate</code> avec <code>domains_from = \"contexts\"</code> \
                les cartographie.",
            heatmap: "Où est la dette",
            heatmap_intro: "Les éléments de dette de chaque domaine par type ; plus l'or d'une case est vif, \
                plus il y en a.",
            fewer: "moins",
            more: "plus",
            of: "sur",
            trends: "Tendances",
            trends_intro: "Les chiffres de chaque run enregistré de <code>retrodoc generate</code>, \
                du plus récent au plus ancien ; un chiffre pire qu'au run précédent est marqué ▼.",
            no_trends: "Les tendances demandent deux runs : chaque <code>retrodoc generate</code> \
                qui change la doc en enregistre un.",
            trend_columns: [
                "Run",
                "Commit",
                "Confiance",
                "Langage métier",
                "Tenus par une fonctionnalité",
                "Cités par un cas d'usage",
                "Éléments de dette",
            ],
            trends_debt: "Éléments de dette par type",
            worse: "pire",
            chart_note: "au fil des runs enregistrés, le plus ancien à gauche ; le tableau ci-dessous \
                donne les chiffres",
        },
        history_page: HistoryTexts {
            intro: "Les runs de <code>retrodoc generate</code> qui ont changé la doc, et ce \
                qu'un run a changé dans les sections d'un autre.",
            runs: "Runs enregistrés",
            no_commit: "aucun commit",
            fewer: "La différence demande deux runs : chaque <code>retrodoc generate</code> qui \
                change la doc en enregistre un.",
            unknown: "Aucun run enregistré ne s'appelle :",
            from: "Depuis",
            to: "Vers",
            ended: "Fin",
            commit: "Commit",
            swap: "Inverser les runs",
            show: "Afficher",
            no_difference: "Les sections de ces deux runs ne diffèrent pas.",
            no_page: "pas de page",
            added: "Ajoutées",
            removed: "Retirées",
            degraded: "Dégradées",
            set_apart: "Nouvellement mises à part",
            no_longer_set_apart: "Plus mises à part",
            confidence: "Confiance",
            business_language: "Langage métier",
        },
        review: ReviewTexts {
            heading: "Relecture",
            not_reviewed: "Non relu",
            by: "par",
            statuses: ["Validé", "Faux", "À revoir"],
            status_label: "Statut",
            no_status: "Pas de relecture",
            comment: "Commentaire",
            save: "Enregistrer",
            saved: "Relecture enregistrée.",
            outdated: OutdatedTexts {
                heading: "Relecture périmée",
                rewritten: "section réécrite depuis la relecture",
                files_changed: "fichiers modifiés depuis la relecture :",
            },
            outdated_badge: "relecture périmée",
            outdated_count: "Périmées",
        },
        errors: ErrorTexts {
            unknown_status: "Statut de relecture inconnu.",
            no_section: "Le dernier run n'a pas cette section.",
            no_repository: "La doc ne porte aucun dépôt où enregistrer.",
            narrative_of_use_case_only: "Seul un cas d'usage a un récit.",
            blank_question: "La question est vide.",
            question_running: "Une question de cette conversation est en cours.",
            no_docs: "Aucune doc à interroger : lancez `retrodoc generate`.",
        },
        edit: EditTexts {
            generated: "Généré par le dernier run",
            outdated: OutdatedTexts {
                heading: "Correction périmée",
                rewritten: "section réécrite depuis la correction",
                files_changed: "fichiers modifiés depuis la correction :",
            },
            badge: "corrigée",
            form_heading: "Corriger cette section",
            labels: ["Nom", "Description", "Récit"],
            form_hint: "Un champ vide, ou laissé tel que généré, garde le texte généré.",
            save: "Enregistrer la correction",
            saved: "Correction enregistrée.",
            revert: "Revenir au texte généré",
        },
        ask: AskTexts {
            link: "Demander",
            hint: "Poser une question sur l’application ; la réponse lit la doc avec un LLM",
            intro: "Posez une question sur l’application : un LLM répond depuis la doc, en nommant les pages sur lesquelles il s’appuie. Chaque question est envoyée avec les précédentes de cette conversation.",
            question: "Votre question",
            send: "Demander",
            keys: "Entrée envoie, Maj+Entrée va à la ligne.",
            suggested: "Questions suggérées",
            purpose_question: "À quoi sert cette application ?",
            domain_question: |name| format!("Que couvre le domaine {name} ?"),
            waiting: "Lecture de la doc pour répondre…",
            start_over: "Recommencer",
            sources: "Sources",
            warnings: "Avertissements",
            no_source: "Aucune source de la doc n’appuie cette réponse.",
            no_answer: "Le modèle n’a donné aucune réponse.",
            failed: "La question n’a pas pu recevoir de réponse :",
            steps: |n| format!("{n} étape{}", if n > 1 { "s" } else { "" }),
            question_cost: "Cette question :",
            session_cost: "Cette conversation :",
            cost: french_cost,
            kinds: ["domaine", "fonctionnalité", "cas d’usage", "fichier"],
            warning: french_warning,
        },
        refusal: french_refusal,
    };

    /// The table of the language of the two-letter `tag`: English for no tag or a language with
    /// no table.
    #[must_use]
    pub fn for_tag(tag: Option<&str>) -> &'static Self {
        match tag {
            Some("fr") => &Self::FRENCH,
            _ => &Self::ENGLISH,
        }
    }

    /// The kind of a hit of the search panel.
    #[must_use]
    pub fn hit_kind(&self, kind: EntryKind) -> &'static str {
        let index = match kind {
            EntryKind::Domain => 0,
            EntryKind::Feature => 1,
            EntryKind::UseCase => 2,
            EntryKind::Rule => 3,
            EntryKind::Glossary => 4,
            EntryKind::Doc => 5,
        };
        self.hit_kinds[index]
    }

    /// The heading of the hits of `kind` on the search page.
    #[must_use]
    pub fn kind_heading(&self, kind: EntryKind) -> &'static str {
        match kind {
            EntryKind::Domain => self.domains,
            EntryKind::Feature => self.doc.features,
            EntryKind::UseCase => self.doc.use_cases,
            EntryKind::Rule => self.doc.business_rules,
            EntryKind::Glossary => self.page_name("/glossary"),
            EntryKind::Doc => self.search_docs_heading,
        }
    }

    /// The name of the page of reference at `href`, its title.
    #[must_use]
    pub fn page_name(&self, href: &str) -> &'static str {
        self.reference_pages
            .iter()
            .find(|page| page.href == href)
            .map_or("", |page| page.name)
    }

    /// `count` with the singular of `unit` or its plural, by the rule of the language: `1 rule`,
    /// `0 rules`, `0 règle`.
    #[must_use]
    pub fn count(&self, count: usize, unit: (&str, &str)) -> String {
        let singular = count == 1 || (count == 0 && self.zero_is_singular);
        format!("{count} {}", if singular { unit.0 } else { unit.1 })
    }

    /// `count` features with their unit.
    #[must_use]
    pub fn features(&self, count: usize) -> String {
        self.count(count, self.feature_unit)
    }

    /// `count` use cases with their unit.
    #[must_use]
    pub fn use_cases(&self, count: usize) -> String {
        self.count(count, self.use_case_unit)
    }

    /// What cites a file: a feature, a use case or a rule.
    #[must_use]
    pub fn citer(&self, kind: CiterKind) -> &'static str {
        match kind {
            CiterKind::Feature => self.feature,
            CiterKind::UseCase => self.use_case,
            CiterKind::Rule => self.rule,
        }
    }

    /// What backs a rule, with its colon: `Code:`.
    #[must_use]
    pub fn evidence(&self, kind: EvidenceKind) -> &'static str {
        match kind {
            EvidenceKind::Code => self.doc.code,
            EvidenceKind::Test => self.doc.test,
            EvidenceKind::Documentation => self.doc.documentation,
        }
    }

    /// What a doc item is.
    #[must_use]
    pub fn item_kind(&self, kind: ItemKind) -> &'static str {
        let at = match kind {
            ItemKind::Domain => 0,
            ItemKind::Feature => 1,
            ItemKind::UseCase => 2,
            ItemKind::Rule => 3,
        };
        self.debt.item_kinds[at]
    }

    /// The priority of a debt item and its parts.
    #[must_use]
    pub fn priority(&self, priority: &Priority) -> String {
        let [commit, commits] = self.debt.commits;
        let [entry_point, entry_points] = [self.debt.counted[0], self.debt.counted[1]];
        (self.debt.priority)(
            priority.value(),
            priority.weight,
            priority.weakness,
            self.count(
                usize::try_from(priority.commits).unwrap_or(usize::MAX),
                (commit, commits),
            ),
            self.count(priority.entry_points, (entry_point, entry_points)),
        )
    }

    /// `count` entry points, or `count` files.
    #[must_use]
    pub fn counted(&self, entry_points: bool, count: usize) -> String {
        let at = 2 * usize::from(!entry_points) + usize::from(count != 1);
        format!("{count} {}", self.debt.counted[at])
    }

    /// The name of a kind of debt.
    #[must_use]
    pub fn debt_kind(&self, kind: DebtKind) -> &'static str {
        DebtKind::ALL
            .iter()
            .position(|k| *k == kind)
            .map_or("", |at| self.debt.kinds[at])
    }

    /// Why a use case is set apart.
    #[must_use]
    pub fn set_apart_reason(&self, reason: SetApartReason) -> &'static str {
        match reason {
            SetApartReason::BackOffice => self.debt.reasons[0],
            SetApartReason::BusinessLanguage => self.debt.reasons[1],
            SetApartReason::Confidence => self.debt.reasons[2],
        }
    }

    /// The label of a confidence badge: its percent, or not scored.
    #[must_use]
    pub fn confidence(&self, percent: Option<&String>) -> String {
        percent.map_or_else(|| self.doc.not_scored.to_string(), String::clone)
    }
}

/// A warning of an answer, as the agent words it.
fn english_warning(warning: &Warning, _: &AskTexts) -> String {
    warning.to_string()
}

/// A warning of an answer, in French; the ids and paths stay as written.
fn french_warning(warning: &Warning, texts: &AskTexts) -> String {
    match warning {
        Warning::LowConfidence { source, confidence } => format!(
            "confiance faible ({:.0} %) dans {} `{}` : le code ne l’appuie peut-être pas",
            confidence * 100.0,
            texts.kind(source.kind),
            source.id
        ),
        Warning::Stale { path, deleted } => format!(
            "`{path}` a été {} depuis la génération de la doc",
            if *deleted { "supprimé" } else { "modifié" }
        ),
    }
}

/// Why a cited file can't be shown, in French; the paths stay as written.
fn french_refusal(error: &SourceError) -> String {
    match error {
        SourceError::NotPlain(path) => {
            format!("Indisponible : `{path}` n'est pas un chemin dans le dépôt.")
        }
        SourceError::NotCited(path) => {
            format!("Indisponible : `{path}` n'est cité par aucune doc.")
        }
        SourceError::Symlink(path) => format!(
            "Indisponible : `{path}` est un lien symbolique, ou passe par un, et les liens ne sont \
             pas suivis."
        ),
        SourceError::Gone(path) => format!("Indisponible : `{path}` n'existe plus dans le dépôt."),
        SourceError::Unreadable(path) => format!("Indisponible : `{path}` n'a pas pu être lu."),
        SourceError::TooLarge { path, size } => format!(
            "Indisponible : `{path}` est trop gros pour être affiché ({size} octets, la limite est \
             {MAX_FILE_BYTES})."
        ),
        SourceError::Binary(path) => format!("Indisponible : `{path}` est un fichier binaire."),
        SourceError::PastEnd { path, total, start } => {
            format!("`{path}` a {total} lignes : la ligne {start} est après la fin.")
        }
        SourceError::Reversed { start, end } => {
            format!("La ligne de fin ({end}) est avant la ligne de début ({start}).")
        }
        SourceError::NoHistory => {
            "Indisponible : ce dépôt n'a pas d'historique git lisible.".to_string()
        }
    }
}

/// `2 calls, 200 tokens in (80 from the cache), 20 out`, then the calls with no usage.
fn english_cost(cost: &CallTotals) -> String {
    let calls = |n: u64| format!("{n} call{}", if n == 1 { "" } else { "s" });
    let mut text = format!("{}, {} tokens in", calls(cost.calls), cost.prompt_tokens);
    if cost.cached_tokens > 0 {
        let _ = write!(text, " ({} from the cache)", cost.cached_tokens);
    }
    let _ = write!(text, ", {} out", cost.completion_tokens);
    if cost.calls_without_usage > 0 {
        let _ = write!(
            text,
            ", {} with no usage reported",
            calls(cost.calls_without_usage)
        );
    }
    text
}

/// `2 appels, 200 tokens en entrée (dont 80 du cache), 20 en sortie`, puis les appels sans usage.
fn french_cost(cost: &CallTotals) -> String {
    let calls = |n: u64| format!("{n} appel{}", if n > 1 { "s" } else { "" });
    let mut text = format!(
        "{}, {} tokens en entrée",
        calls(cost.calls),
        cost.prompt_tokens
    );
    if cost.cached_tokens > 0 {
        let _ = write!(text, " (dont {} du cache)", cost.cached_tokens);
    }
    let _ = write!(text, ", {} en sortie", cost.completion_tokens);
    if cost.calls_without_usage > 0 {
        let _ = write!(
            text,
            ", {} sans usage indiqué",
            calls(cost.calls_without_usage)
        );
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_web_table_of_a_tag_shares_the_words_of_the_docs_table_of_that_tag() {
        for tag in [None, Some("en"), Some("fr"), Some("de")] {
            // The tables are constants, copied where used: compare a word, not an address.
            assert_eq!(
                WebTexts::for_tag(tag).doc.what_happens,
                Texts::for_tag(tag).what_happens,
                "{tag:?}"
            );
        }
    }

    #[test]
    fn zero_takes_the_singular_in_french_only() {
        let unit = ("rule", "rules");
        assert_eq!(WebTexts::ENGLISH.count(0, unit), "0 rules");
        assert_eq!(WebTexts::ENGLISH.count(1, unit), "1 rule");
        assert_eq!(WebTexts::FRENCH.count(0, ("règle", "règles")), "0 règle");
        assert_eq!(WebTexts::FRENCH.count(2, ("règle", "règles")), "2 règles");
    }
}

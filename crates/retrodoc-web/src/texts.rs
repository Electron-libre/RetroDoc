//! The fixed text of the pages, one table per language (`specs/web-ui.md`, WEB-77): the labels,
//! headings and messages around the docs. The words the pages share with the Markdown docs come
//! from the table of `retrodoc-render` ([`WebTexts::doc`]). A language with no table keeps English.

use retrodoc_mcp::EntryKind;
use retrodoc_render::Texts;

use crate::view::{plural, Group, ReferencePage};

/// The fixed text of the pages in one language.
#[derive(Debug)]
pub struct WebTexts {
    /// The words shared with the Markdown docs.
    pub doc: &'static Texts,
    pub read: &'static str,
    pub look_up: &'static str,
    pub search_docs: &'static str,
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
    pub reference_pages: [ReferencePage; 5],
    pub search: &'static str,
    pub search_intro: &'static str,
    pub results_for: &'static str,
    pub search_features: &'static str,
    pub search_glossary: &'static str,
    pub search_docs_heading: &'static str,
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
    /// The results `htmx` swaps under the box before a first `generate`.
    pub no_run_results: &'static str,
    pub no_run_title: &'static str,
    pub no_run_heading: &'static str,
    /// Before `retrodoc generate`, then between it and `retrodoc ui`, then after it.
    pub no_run_text: (&'static str, &'static str, &'static str),
    pub not_found: &'static str,
    pub page_missing: &'static str,
    pub back_to_domains: &'static str,
}

impl WebTexts {
    pub const ENGLISH: Self = Self {
        doc: &Texts::ENGLISH,
        read: "Read",
        look_up: "Look up",
        search_docs: "Search the docs",
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
                unit: ("domain", "domains"),
            },
            ReferencePage {
                href: "/actors",
                name: "Actors",
                hint: "Who uses the application, and which use cases each one starts or takes \
                    part in.",
                group: Group::Read,
                unit: ("actor", "actors"),
            },
            ReferencePage {
                href: "/glossary",
                name: "Glossary",
                hint: "The business concepts of the application, defined, with the use cases \
                    that use them.",
                group: Group::LookUp,
                unit: ("concept", "concepts"),
            },
            ReferencePage {
                href: "/rules",
                name: "Rules",
                hint: "The business rules the code enforces, filtered by kind, domain or feature.",
                group: Group::LookUp,
                unit: ("rule", "rules"),
            },
            ReferencePage {
                href: "/files",
                name: "Files",
                hint: "The source files, with the docs that cite each one and those no doc covers.",
                group: Group::LookUp,
                unit: ("cited file", "cited files"),
            },
        ],
        search: "Search",
        search_intro: "Search the domains, features, use cases, business rules, glossary and \
            existing docs, in the application's own words.",
        results_for: "Results for",
        search_features: "Features",
        search_glossary: "Glossary",
        search_docs_heading: "Existing docs",
        not_documented: "Not documented:",
        nothing_among: "nothing among",
        entries_match: "entries matches",
        no_entry_has: "No entry has",
        one_left_out: "1 entry shares only part of the query.",
        many_left_out: "entries share only part of the query.",
        try_other_words: "Try other words.",
        no_run_results: "<section class=\"results\"><p>Nothing to search yet: run \
            <code>retrodoc generate</code>, then restart <code>retrodoc ui</code>.</p></section>",
        no_run_title: "No run yet",
        no_run_heading: "No docs generated yet",
        no_run_text: ("This repository has no run yet. Run", ", then restart", "."),
        not_found: "Not found",
        page_missing: "This page does not exist.",
        back_to_domains: "Back to the domains",
    };

    pub const FRENCH: Self = Self {
        doc: &Texts::FRENCH,
        read: "Lire",
        look_up: "Consulter",
        search_docs: "Rechercher dans la doc",
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
                unit: ("domaine", "domaines"),
            },
            ReferencePage {
                href: "/actors",
                name: "Acteurs",
                hint: "Qui utilise l'application, et quels cas d'usage chacun lance ou auxquels \
                    il prend part.",
                group: Group::Read,
                unit: ("acteur", "acteurs"),
            },
            ReferencePage {
                href: "/glossary",
                name: "Glossaire",
                hint: "Les concepts métier de l'application, définis, avec les cas d'usage qui \
                    les utilisent.",
                group: Group::LookUp,
                unit: ("concept", "concepts"),
            },
            ReferencePage {
                href: "/rules",
                name: "Règles",
                hint: "Les règles métier que le code applique, filtrées par type, domaine ou \
                    fonctionnalité.",
                group: Group::LookUp,
                unit: ("règle", "règles"),
            },
            ReferencePage {
                href: "/files",
                name: "Fichiers",
                hint: "Les fichiers sources, avec les docs qui citent chacun et ceux qu'aucune \
                    doc ne couvre.",
                group: Group::LookUp,
                unit: ("fichier cité", "fichiers cités"),
            },
        ],
        search: "Recherche",
        search_intro: "Cherchez dans les domaines, fonctionnalités, cas d'usage, règles métier, \
            le glossaire et les docs existantes, avec les mots de l'application.",
        results_for: "Résultats pour",
        search_features: "Fonctionnalités",
        search_glossary: "Glossaire",
        search_docs_heading: "Docs existantes",
        not_documented: "Non documenté :",
        nothing_among: "rien parmi",
        entries_match: "entrées ne correspond à",
        no_entry_has: "Aucune entrée ne contient",
        one_left_out: "1 entrée ne partage qu'une partie de la requête.",
        many_left_out: "entrées ne partagent qu'une partie de la requête.",
        try_other_words: "Essayez d'autres mots.",
        no_run_results: "<section class=\"results\"><p>Rien à chercher pour l'instant : lancez \
            <code>retrodoc generate</code>, puis relancez <code>retrodoc ui</code>.</p></section>",
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

    /// The heading of the hits of `kind` on the search page.
    #[must_use]
    pub fn kind_heading(&self, kind: EntryKind) -> &'static str {
        match kind {
            EntryKind::Domain => self.domains,
            EntryKind::Feature => self.search_features,
            EntryKind::UseCase => self.doc.use_cases,
            EntryKind::Rule => self.doc.business_rules,
            EntryKind::Glossary => self.search_glossary,
            EntryKind::Doc => self.search_docs_heading,
        }
    }

    /// `count` features with their unit.
    #[must_use]
    pub fn features(&self, count: usize) -> String {
        plural(count, self.feature_unit.0, self.feature_unit.1)
    }

    /// `count` use cases with their unit.
    #[must_use]
    pub fn use_cases(&self, count: usize) -> String {
        plural(count, self.use_case_unit.0, self.use_case_unit.1)
    }

    /// The label of a confidence badge: its percent, or not scored.
    #[must_use]
    pub fn confidence(&self, percent: Option<&String>) -> String {
        percent.map_or_else(|| self.doc.not_scored.to_string(), String::clone)
    }
}

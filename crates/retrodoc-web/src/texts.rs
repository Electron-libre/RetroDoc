//! The fixed text of the pages, one table per language (`specs/web-ui.md`, WEB-77): the labels,
//! headings and messages around the docs. The words the pages share with the Markdown docs come
//! from the table of `retrodoc-render` ([`WebTexts::doc`]). A language with no table keeps English.

use retrodoc_core::model::SetApartReason;
use retrodoc_mcp::debt::DebtKind;
use retrodoc_mcp::source::{SourceError, MAX_FILE_BYTES};
use retrodoc_mcp::EntryKind;
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
    pub reference_pages: [ReferencePage; 6],
    pub search: &'static str,
    pub search_intro: &'static str,
    pub results_for: &'static str,
    /// A colon after a label, as the language writes it.
    pub colon: &'static str,
    /// Whether a count of 0 takes the singular, as in French.
    pub zero_is_singular: bool,
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
    /// The lines a page of a file shows: from, to, of how many.
    pub shown: fn(usize, usize, usize) -> String,
    /// Why the lines asked are not shown: the number of lines of the file, the first line asked.
    pub past_end: fn(usize, usize) -> String,
    /// Why a cited file can't be shown.
    pub refusal: fn(&SourceError) -> String,
}

/// The fixed text of the debt page.
#[derive(Debug)]
pub struct DebtTexts {
    pub intro: &'static str,
    /// The name of each kind, in the order of `DebtKind::ALL`.
    pub kinds: [&'static str; 6],
    /// Why a use case is set apart: its back-office feature, its business language, its confidence.
    pub reasons: [&'static str; 3],
    pub set_apart_for: &'static str,
    pub none: &'static str,
    pub none_matches: &'static str,
    pub no_score: &'static str,
    /// The list `htmx` swaps under the form before a first `generate`.
    pub no_run: &'static str,
}

impl WebTexts {
    pub const ENGLISH: Self = Self {
        doc: &Texts::ENGLISH,
        read: "Read",
        look_up: "Look up",
        audit: "Audit",
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
            ReferencePage {
                href: "/debt",
                name: "Debt",
                hint: "The sections of the docs that need a human, filtered by kind or domain.",
                group: Group::Audit,
                unit: ("item", "items"),
            },
        ],
        search: "Search",
        search_intro: "Search the domains, features, use cases, business rules, glossary and \
            existing docs, in the application's own words.",
        results_for: "Results for",
        colon: ":",
        zero_is_singular: false,
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
        kind: "Kind",
        domain: "Domain",
        feature: "Feature",
        use_case: "Use case",
        rule: "Rule",
        rule_feature: "Feature:",
        not_in_the_run: "not in the run",
        no_rule_in_run: "No business rule was found in the code of the last run.",
        no_rule_matches: "No rule matches.",
        no_run_rules: "<section id=\"rule-list\"><p>No rule yet: run <code>retrodoc generate</code>, then restart <code>retrodoc ui</code>.</p></section>",
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
                to document by hand.",
            kinds: [
                "Low confidence",
                "Not scored",
                "Unbacked narrative",
                "Technical language",
                "Set apart",
                "Stale",
            ],
            reasons: ["back-office feature", "business language", "confidence"],
            set_apart_for: "Set apart for:",
            none: "No section of the docs needs a human.",
            none_matches: "No item matches.",
            no_score: "The last run has no confidence score: its use cases are not listed as not \
                scored. Run <code>retrodoc generate</code> again to score them.",
            no_run: "<section id=\"debt-list\"><p>No debt yet: run <code>retrodoc generate</code>, then restart <code>retrodoc ui</code>.</p></section>",
        },
        refusal: SourceError::to_string,
    };

    pub const FRENCH: Self = Self {
        doc: &Texts::FRENCH,
        read: "Lire",
        look_up: "Consulter",
        audit: "Auditer",
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
            ReferencePage {
                href: "/debt",
                name: "Dette",
                hint: "Les sections de la doc qui demandent un humain, filtrées par type ou domaine.",
                group: Group::Audit,
                unit: ("élément", "éléments"),
            },
        ],
        search: "Recherche",
        search_intro: "Cherchez dans les domaines, fonctionnalités, cas d'usage, règles métier, \
            le glossaire et les docs existantes, avec les mots de l'application.",
        results_for: "Résultats pour",
        colon: " :",
        zero_is_singular: true,
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
        kind: "Type",
        domain: "Domaine",
        feature: "Fonctionnalité",
        use_case: "Cas d'usage",
        rule: "Règle",
        rule_feature: "Fonctionnalité :",
        not_in_the_run: "absente du run",
        no_rule_in_run: "Aucune règle métier n'a été trouvée dans le code du dernier run.",
        no_rule_matches: "Aucune règle ne correspond.",
        no_run_rules: "<section id=\"rule-list\"><p>Pas encore de règle : lancez <code>retrodoc generate</code>, puis relancez <code>retrodoc ui</code>.</p></section>",
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
                code ou à documenter à la main.",
            kinds: [
                "Confiance faible",
                "Non évalué",
                "Récit non étayé",
                "Langage technique",
                "Mis à part",
                "Périmé",
            ],
            reasons: ["fonctionnalité back-office", "langage métier", "confiance"],
            set_apart_for: "Mis à part pour :",
            none: "Aucune section de la doc n'a besoin d'un humain.",
            none_matches: "Aucun élément ne correspond.",
            no_score: "Le dernier run n'a aucun score de confiance : ses cas d'usage ne sont pas \
                listés comme non évalués. Relancez <code>retrodoc generate</code> pour les évaluer.",
            no_run: "<section id=\"debt-list\"><p>Pas encore de dette : lancez <code>retrodoc generate</code>, puis relancez <code>retrodoc ui</code>.</p></section>",
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

    /// The name of a kind of debt.
    #[must_use]
    pub fn debt_kind(&self, kind: DebtKind) -> &'static str {
        let at = DebtKind::ALL.iter().position(|k| *k == kind).unwrap_or(0);
        self.debt.kinds[at]
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

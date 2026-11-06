//! What the pages show, computed from the read model: plain data the
//! templates only lay out, so the rules of what a page shows are tested
//! without HTML.

use std::collections::BTreeSet;

use crate::actors::{actors_page, ActorsPage};
use crate::edit::is_edited;
use crate::files::{files_page, FilesPage};
use crate::glossary::{glossary_page, GlossaryPage};
use crate::map::{map_href, maps, Maps};
use crate::review::{Place, ReviewBadge};
use crate::texts::WebTexts;
use crate::Project;
use retrodoc_core::model::{
    ActorKind, BusinessRule, ConfidenceScore, Feature, RuleEvidence, RuleKind, SourceRef, Step,
    UseCase, LOW_CONFIDENCE_THRESHOLD,
};
use retrodoc_mcp::freshness::{FileState, Freshness};
use retrodoc_mcp::history::{self, SavedRun};
use retrodoc_mcp::reviews::Reviews;
use retrodoc_mcp::source::SourceAccess;
use retrodoc_mcp::{ExistingDoc, GeneratedDocs};
use retrodoc_render::{anchor, source_ref_label, DocLinks, ItemRef};

/// The docs the interface serves, `None` before a first `generate`, and the
/// frame every page shows, built once since the docs don't change.
pub struct Site {
    texts: &'static WebTexts,
    docs: Option<GeneratedDocs>,
    links: DocLinks,
    shell: Shell,
    purpose: Option<String>,
    users: Vec<String>,
    start: Vec<StartLink>,
    source: Option<SourceAccess>,
    files: Option<FilesPage>,
    collected: Vec<ExistingDoc>,
    glossary: Option<GlossaryPage>,
    actors: Option<ActorsPage>,
    maps: Option<Maps>,
    history: Vec<SavedRun>,
}

impl Site {
    #[must_use]
    pub fn new(
        project: Project,
        docs: Option<GeneratedDocs>,
        links: DocLinks,
        collected: Vec<ExistingDoc>,
    ) -> Self {
        let texts = WebTexts::for_tag(project.lang);
        let shell = Shell {
            name: project.name,
            lang: project.lang,
            read: reference_pages(texts, Group::Read),
            look_up: reference_pages(texts, Group::LookUp),
            audit: reference_pages(texts, Group::Audit),
            ask: false,
            tree: docs.as_ref().map(tree).unwrap_or_default(),
        };
        // The files the debt lists are served too, though no doc cites them (`specs/debt-view.md`).
        let source = docs
            .as_ref()
            .and_then(GeneratedDocs::source_access_with_debt);
        let files = docs.as_ref().map(files_page);
        let glossary = docs.as_ref().map(glossary_page);
        let actors = docs.as_ref().map(|docs| actors_page(docs, texts));
        let maps = docs.as_ref().map(|docs| maps(docs, texts));
        // A snapshot is saved by a `generate`, which leaves docs: none without them.
        let history = docs
            .as_ref()
            .and_then(GeneratedDocs::repo_root)
            .map(history::load)
            .unwrap_or_default();
        let start = match (&docs, &files, &glossary, &actors) {
            (Some(docs), Some(files), Some(glossary), Some(actors)) => {
                start_links(texts, docs, files, glossary, actors)
            }
            _ => Vec::new(),
        };
        Self {
            texts,
            docs,
            links,
            shell,
            purpose: project.purpose.filter(|purpose| !purpose.trim().is_empty()),
            users: project
                .users
                .into_iter()
                .filter(|user| !user.trim().is_empty())
                .collect(),
            start,
            source,
            files,
            collected,
            glossary,
            actors,
            maps,
            history,
        }
    }

    /// The saved runs, oldest first.
    #[must_use]
    pub fn history(&self) -> &[SavedRun] {
        &self.history
    }

    /// The maps of the domains, `None` with no run.
    #[must_use]
    pub fn maps(&self) -> Option<&Maps> {
        self.maps.as_ref()
    }

    /// The actors page, `None` with no run.
    #[must_use]
    pub fn actors(&self) -> Option<&ActorsPage> {
        self.actors.as_ref()
    }

    /// The glossary page, `None` with no run.
    #[must_use]
    pub fn glossary(&self) -> Option<&GlossaryPage> {
        self.glossary.as_ref()
    }

    /// The project's own Markdown docs, served with or without a run.
    #[must_use]
    pub fn collected(&self) -> &[ExistingDoc] {
        &self.collected
    }

    /// The files page, `None` with no run.
    #[must_use]
    pub fn files(&self) -> Option<&FilesPage> {
        self.files.as_ref()
    }

    /// The files the docs cite, `None` with no run.
    #[must_use]
    pub fn source(&self) -> Option<&SourceAccess> {
        self.source.as_ref()
    }

    #[must_use]
    pub fn docs(&self) -> Option<&GeneratedDocs> {
        self.docs.as_ref()
    }

    #[must_use]
    pub fn links(&self) -> &DocLinks {
        &self.links
    }

    /// The site whose header links to the ask page when `ask` (WEB-101).
    #[must_use]
    pub fn with_ask_link(mut self, ask: bool) -> Self {
        self.shell.ask = ask;
        self
    }

    /// The fixed text of the pages, in the language of the docs.
    #[must_use]
    pub fn texts(&self) -> &'static WebTexts {
        self.texts
    }

    /// The header and the tree of every page.
    #[must_use]
    pub fn shell(&self) -> &Shell {
        &self.shell
    }

    /// The purpose of the product brief, `None` with no brief or an empty one.
    #[must_use]
    pub fn purpose(&self) -> Option<&str> {
        self.purpose.as_deref()
    }

    /// The users of the product brief, the blank ones left out.
    #[must_use]
    pub fn users(&self) -> &[String] {
        &self.users
    }

    /// The pages the home page points a first-time reader to, with what each
    /// holds and its count; empty with no run. The count of the debt items is
    /// that of the debt page now: its stale items are judged at each call, so it
    /// reads the cited files, and the reviews.
    #[must_use]
    pub fn start_links(&self) -> Vec<StartLink> {
        let debt = self.docs.as_ref().map_or(0, |docs| {
            let reviews = crate::review::load(docs).unwrap_or_default();
            retrodoc_mcp::debt::debt(docs, &reviews).items.len()
        });
        self.start
            .iter()
            .map(|link| StartLink {
                label: if link.page.href == "/debt" {
                    self.texts.count(debt, link.unit)
                } else {
                    link.label.clone()
                },
                page: link.page,
                unit: link.unit,
            })
            .collect()
    }
}

/// The pages of reference with their counts as the pages they link to show
/// them: the domains, a row per actor and kind, the concepts, the rules, the
/// cited files; the debt items are counted at each call ([`Site::start_links`]). A page with no
/// unit, the dashboard, counts nothing and is no line of it.
fn start_links(
    texts: &'static WebTexts,
    docs: &GeneratedDocs,
    files: &FilesPage,
    glossary: &GlossaryPage,
    actors: &ActorsPage,
) -> Vec<StartLink> {
    let actors = actors.groups.iter().map(|group| group.rows.len()).sum();
    texts
        .reference_pages
        .iter()
        .filter_map(|page| page.unit.map(|unit| (page, unit)))
        .zip([
            docs.domains().len(),
            actors,
            glossary.concepts.len(),
            docs.rules().len(),
            files.cited,
            0,
        ])
        .map(|((page, unit), count)| StartLink {
            label: texts.count(count, unit),
            page,
            unit,
        })
        .collect()
}

/// How a reader uses a page of reference: reads through it, looks things up, or audits the docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Read,
    LookUp,
    Audit,
}

/// The pages of reference of `group`, in their order.
fn reference_pages(texts: &'static WebTexts, group: Group) -> Vec<&'static ReferencePage> {
    texts
        .reference_pages
        .iter()
        .filter(|page| page.group == group)
        .collect()
}

/// A page of reference beside the tree of the docs, as the header and the
/// home page point to it.
#[derive(Debug)]
pub struct ReferencePage {
    pub href: &'static str,
    pub name: &'static str,
    /// What the page holds, in one line.
    pub hint: &'static str,
    pub group: Group,
    /// What its count counts, singular then plural; `None` for a page that counts nothing, left
    /// out of the home page.
    pub unit: Option<(&'static str, &'static str)>,
}

/// A page of reference on the home page, with its count.
pub struct StartLink {
    pub page: &'static ReferencePage,
    /// The count with its unit.
    pub label: String,
    pub unit: (&'static str, &'static str),
}

/// What every page shows around its content: the name of the application,
/// the links of the header grouped by use, and the tree of the docs, empty
/// with no run.
pub struct Shell {
    pub name: String,
    /// The `lang` of the pages, `None` for a language with no tag.
    pub lang: Option<&'static str>,
    /// The pages one reads through: the map, the actors.
    pub read: Vec<&'static ReferencePage>,
    /// The pages one looks things up in: the glossary, the rules, the files.
    pub look_up: Vec<&'static ReferencePage>,
    /// The pages one audits the docs with: the debt, the dashboard.
    pub audit: Vec<&'static ReferencePage>,
    /// Whether the header links to the ask page (`retrodoc ui --ask`, WEB-101).
    pub ask: bool,
    pub tree: Vec<TreeDomain>,
}

pub struct TreeDomain {
    pub slug: String,
    pub name: String,
    pub features: Vec<TreeFeature>,
}

pub struct TreeFeature {
    pub slug: String,
    pub name: String,
    pub use_cases: Vec<TreeUseCase>,
}

pub struct TreeUseCase {
    pub slug: String,
    pub name: String,
}

/// The domains, their features and the use cases of each, in the order of the run.
#[must_use]
pub fn tree(docs: &GeneratedDocs) -> Vec<TreeDomain> {
    docs.domains()
        .iter()
        .map(|domain| TreeDomain {
            slug: domain.slug.clone(),
            name: domain.name.clone(),
            features: docs
                .features_of(&domain.slug)
                .map(|feature| TreeFeature {
                    slug: feature.slug.clone(),
                    name: feature.name.clone(),
                    use_cases: docs
                        .use_cases_of(&feature.slug)
                        .map(|u| TreeUseCase {
                            slug: u.slug.clone(),
                            name: u.name.clone(),
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}

/// A confidence score as a badge: `80%`, or `None` not scored, flagged low under
/// [`LOW_CONFIDENCE_THRESHOLD`], with why the model gave it.
pub struct Confidence {
    pub percent: Option<String>,
    pub low: bool,
    pub rationale: Option<String>,
}

impl Confidence {
    #[must_use]
    pub fn of(score: Option<&ConfidenceScore>) -> Self {
        match score {
            None => Self {
                percent: None,
                low: false,
                rationale: None,
            },
            Some(s) => Self {
                percent: Some(format!("{:.0}%", s.value * 100.0)),
                low: s.value < LOW_CONFIDENCE_THRESHOLD,
                rationale: s
                    .rationale
                    .as_deref()
                    .map(str::trim)
                    .filter(|r| !r.is_empty())
                    .map(str::to_string),
            },
        }
    }
}

/// A domain in the list of the home page.
pub struct DomainRow {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub confidence: Confidence,
    pub review: Option<ReviewBadge>,
    /// It holds a human edit (WEB-100).
    pub edited: bool,
    /// A file one of its features cites was modified or deleted since the run.
    pub stale: bool,
    pub features: usize,
    pub use_cases: usize,
}

/// Every domain of the run, in its order, with the status of its review among
/// `reviews`. Reads the files the features cite to tell whether they changed
/// since the run.
#[must_use]
pub fn domain_rows(docs: &GeneratedDocs, reviews: &Reviews) -> Vec<DomainRow> {
    docs.domains()
        .iter()
        .map(|domain| {
            let features: Vec<_> = docs.features_of(&domain.slug).collect();
            let cited: BTreeSet<&str> = features
                .iter()
                .flat_map(|f| &f.source_paths)
                .map(String::as_str)
                .collect();
            let stale = docs
                .freshness()
                .is_some_and(|freshness| !freshness.stale(cited).is_empty());
            let place = Place::Domain(domain.slug.clone());
            DomainRow {
                slug: domain.slug.clone(),
                name: domain.name.clone(),
                description: domain.description.clone(),
                confidence: Confidence::of(domain.confidence.as_ref()),
                review: ReviewBadge::of_place(docs, reviews, &place),
                edited: is_edited(docs, &place),
                stale,
                features: features.len(),
                use_cases: features
                    .iter()
                    .map(|f| docs.use_cases_of(&f.slug).count())
                    .sum(),
            }
        })
        .collect()
}

/// A link of a breadcrumb.
pub struct Link {
    pub href: String,
    pub name: String,
}

/// A feature or a use case in the list of the page above it.
pub struct ItemRow {
    pub href: String,
    pub name: String,
    pub confidence: Confidence,
    pub review: Option<ReviewBadge>,
    /// It holds a human edit (WEB-100).
    pub edited: bool,
    pub description: String,
}

/// A rule in the list of a domain, with the feature it belongs to.
pub struct DomainRule {
    pub statement: String,
    pub kind: RuleKind,
    pub confidence: Confidence,
    pub feature: Link,
}

/// A section of the project's own docs that documents or contradicts an item.
pub struct DocItem {
    /// The page of the collected doc, at the section.
    pub href: String,
    /// From the repository root.
    pub path: String,
    pub heading: Option<String>,
    /// What the section states against the item.
    pub contradicts: Option<String>,
}

/// The page of a domain, its sections in the order of its Markdown page.
pub struct DomainPage {
    pub name: String,
    pub description: String,
    /// Its map.
    pub map: String,
    pub confidence: Confidence,
    pub features: Vec<ItemRow>,
    /// The back-office features, set apart.
    pub back_office: Vec<ItemRow>,
    pub rules: Vec<DomainRule>,
    pub docs: Vec<DocItem>,
}

pub(crate) fn domain_href(slug: &str) -> String {
    format!("/domains/{slug}")
}

pub(crate) fn feature_href(feature: &Feature) -> String {
    format!("/domains/{}/{}", feature.domain_slug, feature.slug)
}

pub(crate) fn rule_anchor(slug: &str) -> String {
    format!("rule-{slug}")
}

/// The item of the rule `slug` on the page of its `feature`.
pub(crate) fn rule_href(feature: &Feature, slug: &str) -> String {
    format!("{}#{}", feature_href(feature), rule_anchor(slug))
}

pub(crate) fn use_case_href(feature: &Feature, use_case: &UseCase) -> String {
    format!("{}/{}", feature_href(feature), use_case.slug)
}

fn feature_row(docs: &GeneratedDocs, feature: &Feature, reviews: &Reviews) -> ItemRow {
    let place = Place::Feature {
        domain: feature.domain_slug.clone(),
        slug: feature.slug.clone(),
    };
    ItemRow {
        href: feature_href(feature),
        name: feature.name.clone(),
        confidence: Confidence::of(feature.confidence.as_ref()),
        review: ReviewBadge::of_place(docs, reviews, &place),
        edited: is_edited(docs, &place),
        description: feature.description.trim().to_string(),
    }
}

/// `path` as the path of a URL: every byte but the unreserved ones and `/`
/// percent-encoded, so a `#`, a `?` or a space stays in the path.
pub(crate) fn url_path(path: &str) -> String {
    use std::fmt::Write as _;
    let mut url = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/".contains(&byte) {
            url.push(char::from(byte));
        } else {
            let _ = write!(url, "%{byte:02X}");
        }
    }
    url
}

/// A citation of code, linked to the page of its file.
pub struct CodeLink {
    pub href: String,
    /// `path:start-end`, `path:start` or `path`.
    pub label: String,
}

impl CodeLink {
    fn of(source: &SourceRef) -> Self {
        Self {
            href: match source.lines() {
                Some((start, end)) => code_href(&source.path, Some(start), Some(end)),
                None => code_href(&source.path, None, None),
            },
            label: source_ref_label(source),
        }
    }

    fn file(path: &str) -> Self {
        Self {
            href: code_href(path, None, None),
            label: path.to_string(),
        }
    }
}

/// The page of the file `path`, at the lines `start` to `end` when given.
#[must_use]
pub fn code_href(path: &str, start: Option<u32>, end: Option<u32>) -> String {
    let path = url_path(path.strip_prefix("./").unwrap_or(path));
    match (start, end) {
        (Some(s), Some(e)) if e > s => format!("/code/{path}?lines={s}-{e}#L{s}"),
        (Some(s), _) => format!("/code/{path}?lines={s}#L{s}"),
        _ => format!("/code/{path}"),
    }
}

/// The page of the collected doc `path`, at its section `heading` when given.
pub(crate) fn doc_section_href(path: &str, heading: Option<&str>) -> String {
    match heading {
        Some(h) => format!("/docs/{}#{}", url_path(path), anchor(h)),
        None => format!("/docs/{}", url_path(path)),
    }
}

/// The doc sections behind `item`, linked to the page of the collected doc.
fn doc_items(links: &DocLinks, item: &ItemRef) -> Vec<DocItem> {
    links
        .of(item)
        .iter()
        .map(|doc| DocItem {
            href: doc_section_href(&doc.path, doc.heading.as_deref()),
            path: doc.path.clone(),
            heading: doc.heading.clone(),
            contradicts: doc.contradicts.clone(),
        })
        .collect()
}

/// The page of the domain `slug`, its features with the status of their review
/// among `reviews`; `None` when the run has no such domain.
#[must_use]
pub fn domain_page(
    docs: &GeneratedDocs,
    links: &DocLinks,
    reviews: &Reviews,
    slug: &str,
) -> Option<DomainPage> {
    let domain = docs.domains().iter().find(|d| d.slug == slug)?;
    let features: Vec<&Feature> = docs.features_of(slug).collect();
    let (back_office, business): (Vec<&Feature>, Vec<&Feature>) =
        features.iter().partition(|f| f.back_office);
    let rules = features
        .iter()
        .flat_map(|f| {
            docs.rules()
                .iter()
                .filter(|r| r.belongs_to_feature(&f.slug))
                .map(move |r| DomainRule {
                    statement: r.statement.clone(),
                    kind: r.kind,
                    confidence: Confidence::of(r.confidence.as_ref()),
                    feature: Link {
                        href: feature_href(f),
                        name: f.name.clone(),
                    },
                })
        })
        .collect();
    Some(DomainPage {
        name: domain.name.clone(),
        description: domain.description.trim().to_string(),
        map: map_href(slug),
        confidence: Confidence::of(domain.confidence.as_ref()),
        features: business
            .into_iter()
            .map(|f| feature_row(docs, f, reviews))
            .collect(),
        back_office: back_office
            .into_iter()
            .map(|f| feature_row(docs, f, reviews))
            .collect(),
        rules,
        docs: doc_items(links, &ItemRef::Domain(slug.to_string())),
    })
}

/// A file a page cites that changed since the run.
#[derive(Debug)]
pub struct StaleFile {
    pub path: String,
    /// Deleted, else modified.
    pub deleted: bool,
}

/// The files among `paths` modified or deleted since the run; none when the
/// docs carry no freshness. Reads the files.
pub(crate) fn stale_files<'a>(
    freshness: Option<&Freshness>,
    paths: impl IntoIterator<Item = &'a str>,
) -> Vec<StaleFile> {
    freshness
        .map(|f| f.stale(paths))
        .unwrap_or_default()
        .into_iter()
        .map(|(path, state)| StaleFile::of(path, state))
        .collect()
}

impl StaleFile {
    pub(crate) fn of(path: &str, state: FileState) -> Self {
        Self {
            path: path.to_string(),
            deleted: state == FileState::Deleted,
        }
    }
}

/// What backs a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceKind {
    Code,
    Test,
    Documentation,
}

/// What backs a rule: `Code` and the cited lines, `Test` and its phrase,
/// `Documentation` and its section, with the file it comes from.
pub struct Evidence {
    pub kind: EvidenceKind,
    /// The page of the cited lines of code.
    pub code: Option<String>,
    pub text: String,
    pub source: Option<String>,
}

impl Evidence {
    pub(crate) fn of(evidence: &RuleEvidence) -> Self {
        match evidence {
            RuleEvidence::Code { source } => Self {
                kind: EvidenceKind::Code,
                code: Some(CodeLink::of(source).href),
                text: source_ref_label(source),
                source: None,
            },
            RuleEvidence::Test { phrase, path } => Self {
                kind: EvidenceKind::Test,
                code: None,
                text: phrase.clone(),
                source: Some(path.clone()),
            },
            RuleEvidence::Doc { path, heading } => Self {
                kind: EvidenceKind::Documentation,
                code: None,
                text: heading.clone(),
                source: Some(path.clone()),
            },
        }
    }
}

/// A rule on the page of its feature or of a use case it applies to.
pub struct RuleView {
    /// The `id` of its item: `rule-<slug>`, what a search hit links to.
    pub anchor: String,
    pub statement: String,
    pub kind: RuleKind,
    pub confidence: Confidence,
    pub entities: Vec<String>,
    /// The use cases of the page it applies to; none on a use case page.
    pub use_cases: Vec<Link>,
    pub evidence: Vec<Evidence>,
    pub docs: Vec<DocItem>,
}

fn rule_view(
    rule: &BusinessRule,
    feature: &Feature,
    use_cases: &[&UseCase],
    links: &DocLinks,
) -> RuleView {
    RuleView {
        anchor: rule_anchor(&rule.slug),
        statement: rule.statement.clone(),
        kind: rule.kind,
        confidence: Confidence::of(rule.confidence.as_ref()),
        entities: rule.entities.clone(),
        use_cases: use_cases
            .iter()
            .filter(|u| rule.use_cases.contains(&u.slug))
            .map(|u| Link {
                href: use_case_href(feature, u),
                name: u.name.clone(),
            })
            .collect(),
        evidence: rule.evidence.iter().map(Evidence::of).collect(),
        docs: doc_items(links, &ItemRef::Rule(rule.slug.clone())),
    }
}

/// The page of a feature, its sections in the order of its Markdown page,
/// then its entry points.
pub struct FeaturePage {
    pub crumbs: Vec<Link>,
    pub name: String,
    pub description: String,
    pub confidence: Confidence,
    pub back_office: bool,
    pub stale: Vec<StaleFile>,
    /// None at all: none could be derived from the code.
    pub no_use_case: bool,
    pub use_cases: Vec<ItemRow>,
    /// The use cases `UseCase::is_technical` sets apart.
    pub technical: Vec<ItemRow>,
    pub rules: Vec<RuleView>,
    pub docs: Vec<DocItem>,
    pub source_files: Vec<CodeLink>,
    pub entry_points: Vec<String>,
}

fn crumbs_to(docs: &GeneratedDocs, feature: Option<&Feature>, domain_slug: &str) -> Vec<Link> {
    let domain_name = docs
        .domains()
        .iter()
        .find(|d| d.slug == domain_slug)
        .map_or_else(|| domain_slug.to_string(), |d| d.name.clone());
    let mut crumbs = vec![Link {
        href: domain_href(domain_slug),
        name: domain_name,
    }];
    if let Some(f) = feature {
        crumbs.push(Link {
            href: feature_href(f),
            name: f.name.clone(),
        });
    }
    crumbs
}

/// The feature `slug` of the domain `domain`, `None` when the run has no such
/// feature in that domain.
#[must_use]
pub fn find_feature<'a>(docs: &'a GeneratedDocs, domain: &str, slug: &str) -> Option<&'a Feature> {
    docs.features()
        .iter()
        .find(|f| f.slug == slug && f.domain_slug == domain)
}

/// The page of the feature `slug` of the domain `domain`, its use cases with
/// the status of their review among `reviews`; `None` when the run has no such
/// feature there. Reads the files it cites to tell whether they changed since
/// the run.
#[must_use]
pub fn feature_page(
    docs: &GeneratedDocs,
    links: &DocLinks,
    reviews: &Reviews,
    domain: &str,
    slug: &str,
) -> Option<FeaturePage> {
    let feature = find_feature(docs, domain, slug)?;
    let use_cases: Vec<&UseCase> = docs.use_cases_of(slug).collect();
    let row = |u: &&UseCase| {
        let place = Place::UseCase {
            domain: domain.to_string(),
            feature: slug.to_string(),
            slug: u.slug.clone(),
        };
        ItemRow {
            href: use_case_href(feature, u),
            name: u.name.clone(),
            confidence: Confidence::of(u.confidence.as_ref()),
            review: ReviewBadge::of_place(docs, reviews, &place),
            edited: is_edited(docs, &place),
            description: u.description.trim().to_string(),
        }
    };
    let (technical, business): (Vec<&UseCase>, Vec<&UseCase>) = use_cases
        .iter()
        .partition(|u| u.is_technical(feature.back_office));
    Some(FeaturePage {
        crumbs: crumbs_to(docs, None, domain),
        name: feature.name.clone(),
        description: feature.description.trim().to_string(),
        confidence: Confidence::of(feature.confidence.as_ref()),
        back_office: feature.back_office,
        stale: stale_files(
            docs.freshness(),
            feature.source_paths.iter().map(String::as_str),
        ),
        no_use_case: use_cases.is_empty(),
        use_cases: business.iter().map(row).collect(),
        technical: technical.iter().map(row).collect(),
        rules: docs
            .rules()
            .iter()
            .filter(|r| r.belongs_to_feature(slug))
            .map(|r| rule_view(r, feature, &use_cases, links))
            .collect(),
        docs: doc_items(links, &ItemRef::Feature(slug.to_string())),
        source_files: feature
            .source_paths
            .iter()
            .map(|path| CodeLink::file(path))
            .collect(),
        entry_points: feature.entry_points.clone(),
    })
}

/// A step of a use case.
pub struct StepView {
    pub order: u32,
    pub actor: String,
    pub human: bool,
    pub action: String,
    pub detail: Option<String>,
    /// The cited lines, `path:start-end`.
    pub cited: Vec<CodeLink>,
}

impl StepView {
    fn of(step: &Step) -> Self {
        Self {
            order: step.order,
            actor: step.actor.name.clone(),
            human: step.actor.kind == ActorKind::Human,
            action: step.label().to_string(),
            detail: step.detail().map(str::to_string),
            cited: step.source_refs.iter().map(CodeLink::of).collect(),
        }
    }
}

/// The page of a use case, its sections in the order of its Markdown page.
pub struct UseCasePage {
    pub crumbs: Vec<Link>,
    pub name: String,
    pub description: String,
    pub confidence: Confidence,
    /// `None` until the vocabulary pass scored it.
    pub business_language: Option<Confidence>,
    pub primary_actor: Option<String>,
    /// The names of the entry points that trigger it.
    pub triggered_by: Vec<String>,
    pub stale: Vec<StaleFile>,
    pub narrative: Option<String>,
    /// The confidence pass judged the narrative unsupported by the cited code.
    pub unbacked: bool,
    pub unbacked_rationale: Option<String>,
    pub rules: Vec<RuleView>,
    pub steps: Vec<StepView>,
    pub diagram: Option<String>,
    pub docs: Vec<DocItem>,
    /// The use cases of its feature before and after it, in the order of the run.
    pub previous: Option<Link>,
    pub next: Option<Link>,
}

/// The page of the use case `slug` of the feature `feature` of the domain
/// `domain`, `None` when the run has no such use case there. Reads the files
/// its steps cite to tell whether they changed since the run.
#[must_use]
pub fn use_case_page(
    docs: &GeneratedDocs,
    links: &DocLinks,
    domain: &str,
    feature: &str,
    slug: &str,
) -> Option<UseCasePage> {
    let owner = find_feature(docs, domain, feature)?;
    let siblings: Vec<&UseCase> = docs.use_cases_of(feature).collect();
    let at = siblings.iter().position(|u| u.slug == slug)?;
    let use_case = siblings[at];
    let link = |sibling: &UseCase| Link {
        href: use_case_href(owner, sibling),
        name: sibling.name.clone(),
    };
    let cited = retrodoc_mcp::debt::use_case_cited_paths(use_case);
    // As the debt report counts them: a narrative partly backed states what
    // the cited lines do not show, mostly true of the code around them.
    let unbacked = use_case
        .narrative_check
        .as_ref()
        .filter(|check| check.value <= 0.0);
    Some(UseCasePage {
        crumbs: crumbs_to(docs, Some(owner), domain),
        name: use_case.name.clone(),
        description: use_case.description.trim().to_string(),
        confidence: Confidence::of(use_case.confidence.as_ref()),
        business_language: use_case
            .business_language
            .as_ref()
            .map(|s| Confidence::of(Some(s))),
        primary_actor: use_case.primary_actor.clone(),
        triggered_by: use_case
            .entry_point_names()
            .into_iter()
            .map(str::to_string)
            .collect(),
        stale: stale_files(docs.freshness(), cited),
        narrative: use_case
            .narrative
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_string),
        unbacked: unbacked.is_some(),
        unbacked_rationale: unbacked.and_then(|check| Confidence::of(Some(check)).rationale),
        rules: docs
            .rules()
            .iter()
            .filter(|r| r.belongs_to_feature(feature) && r.use_cases.contains(&use_case.slug))
            .map(|r| rule_view(r, owner, &[], links))
            .collect(),
        steps: use_case.steps.iter().map(StepView::of).collect(),
        diagram: use_case
            .diagram_mermaid
            .as_deref()
            .map(str::trim_end)
            .map(str::to_string),
        docs: doc_items(
            links,
            &ItemRef::UseCase {
                feature: feature.to_string(),
                use_case: slug.to_string(),
            },
        ),
        previous: at.checked_sub(1).map(|i| link(siblings[i])),
        next: siblings.get(at + 1).map(|u| link(u)),
    })
}

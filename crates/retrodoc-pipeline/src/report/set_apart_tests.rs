use super::*;

use retrodoc_core::model::SetApartReason;

#[test]
fn the_report_counts_what_the_docs_set_apart() {
    let mut admin = Feature {
        slug: "admin-tools".into(),
        domain_slug: "ops".into(),
        name: "Admin".into(),
        description: String::new(),
        technical: false,
        source_paths: Vec::new(),
        entry_points: Vec::new(),
        back_office: true,
        confidence: None,
    };
    let mut business = admin.clone();
    business.slug = "pay".into();
    business.back_office = false;
    admin.back_office = true;
    let uc = |slug: &str, feature: &str, business_language: f32| UseCase {
        slug: slug.into(),
        feature_slug: feature.into(),
        name: slug.into(),
        description: String::new(),
        steps: Vec::new(),
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: None,
        business_language: Some(ConfidenceScore::new(business_language, None)),
        diagram_mermaid: None,
        confidence: None,
        narrative_check: None,
    };
    let use_cases = [
        uc("reimport", "admin-tools", 0.95),
        uc("pay", "pay", 0.95),
        uc("helper", "pay", 0.7),
    ];

    let report = build_report(&DomainMap::default(), &[admin, business], &use_cases);

    assert_eq!(
        (
            report.set_apart_use_cases.len(),
            report.back_office_features
        ),
        (2, 1)
    );
    let listed: Vec<(String, &[SetApartReason])> = report
        .set_apart_use_cases
        .iter()
        .map(|s| (s.section.label(), s.reasons.as_slice()))
        .collect();
    assert_eq!(
        listed,
        [
            (
                "ops/admin-tools/reimport".to_string(),
                &[SetApartReason::BackOffice][..]
            ),
            (
                "ops/pay/helper".to_string(),
                &[SetApartReason::BusinessLanguage][..]
            ),
        ]
    );
    assert!(report
        .to_markdown()
        .contains("2 use case(s) and 1 back-office feature(s)"));
    let none = build_report(&DomainMap::default(), &[], &[uc("pay", "pay", 0.95)]);
    assert!(!none.to_markdown().contains("Set apart in the docs"));
}

#[test]
fn the_sections_of_the_report_name_their_domain_feature_and_use_case() {
    let feature = Feature {
        slug: "pay".into(),
        domain_slug: "billing".into(),
        name: "Pay".into(),
        description: String::new(),
        technical: false,
        source_paths: Vec::new(),
        entry_points: Vec::new(),
        back_office: false,
        confidence: Some(ConfidenceScore::new(0.2, None)),
    };
    let uc = |slug: &str, feature: &str, confidence: Option<f32>| UseCase {
        slug: slug.into(),
        feature_slug: feature.into(),
        name: slug.into(),
        description: String::new(),
        steps: Vec::new(),
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: None,
        business_language: None,
        diagram_mermaid: None,
        confidence: confidence.map(|c| ConfidenceScore::new(c, None)),
        narrative_check: None,
    };
    let report = build_report(
        &DomainMap::default(),
        &[feature],
        &[
            uc("refund", "pay", Some(0.1)),
            uc("void", "pay", None),
            uc("lost", "gone", None),
        ],
    );

    assert_eq!(
        report.weak_sections[0].section,
        Section {
            domain: Some("billing".into()),
            feature: "pay".into(),
            use_case: Some("refund".into()),
        }
    );
    let unscored: Vec<String> = report
        .unscored_use_cases
        .iter()
        .map(Section::label)
        .collect();
    // A use case whose feature is not in the run has no domain.
    assert_eq!(unscored, ["billing/pay/void", "?/gone/lost"]);
    assert_eq!(report.unscored_use_cases[1].domain, None);
}

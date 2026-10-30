use super::*;

#[test]
fn the_report_counts_what_the_docs_set_apart() {
    let mut admin = Feature {
        slug: "admin-tools".into(),
        domain_slug: "ops".into(),
        name: "Admin".into(),
        description: String::new(),
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
        (report.set_apart_use_cases, report.back_office_features),
        (2, 1)
    );
    assert!(report
        .to_markdown()
        .contains("2 use case(s) and 1 back-office feature(s)"));
    let none = build_report(&DomainMap::default(), &[], &[uc("pay", "pay", 0.95)]);
    assert!(!none.to_markdown().contains("Set apart in the docs"));
}

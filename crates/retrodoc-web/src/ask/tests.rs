use super::*;

// spec: WAK-11 (internal: a use case outside the run is named only by artifacts of an older run)
#[test]
fn a_use_case_whose_id_names_no_domain_has_no_link() {
    let texts = &WebTexts::ENGLISH.ask;
    let source = |id: &str| Source {
        kind: SourceKind::UseCase,
        id: id.into(),
    };

    let orphan = source_link(&source("pay-invoice/by-card"), texts);
    let placed = source_link(&source("billing/pay-invoice/by-card"), texts);

    assert_eq!(orphan.href, None);
    assert_eq!(
        placed.href.as_deref(),
        Some("/domains/billing/pay-invoice/by-card")
    );
}

// spec: WAK-20 (internal: 33 sessions through the router would only count cookies)
#[test]
fn past_the_bound_the_session_used_least_recently_is_dropped() {
    let mut sessions = Sessions::default();
    let (first, _) = sessions.open(None);
    let (second, _) = sessions.open(None);
    for _ in 2..MAX_SESSIONS {
        sessions.open(None);
    }
    // The first, used again, is no longer the least recent: the second goes.
    assert!(sessions.get(Some(&first)).is_some());
    sessions.open(None);

    assert!(sessions.get(Some(&first)).is_some());
    assert!(sessions.get(Some(&second)).is_none());
    assert!(sessions.open(Some(&second)).1);
    assert_eq!(sessions.0.len(), MAX_SESSIONS);
}

// spec: WAK-27 (internal: the link builder alone decides the encoding of a cited path)
#[test]
fn a_file_source_links_to_its_path_percent_encoded() {
    let file = Source {
        kind: SourceKind::File,
        id: "app/my order#1.rb".into(),
    };

    let link = source_link(&file, &WebTexts::ENGLISH.ask);

    assert_eq!(link.href.as_deref(), Some("/code/app/my%20order%231.rb"));
}

// spec: STL-5 (internal: a session dropped while its question runs needs 33 sessions and a task stopped)
#[test]
fn a_settling_session_started_again_still_settles_its_section() {
    let section = Section::new(retrodoc_docs::reviews::SectionKind::Feature, "list-orders");
    let running = |settled: Option<Section>| {
        Arc::new(Running {
            number: 3,
            question: "Help me settle it.".into(),
            message: "Help me settle it.".into(),
            settled,
            progress: watch::Sender::new(Progress::default()),
        })
    };
    let mut sessions = Sessions::default();

    let settling = sessions.reopen("a", &running(Some(section.clone())));
    assert_eq!(settling.conversation.settled(), Some(&section));
    assert_eq!(settling.asked, 3);
    let plain = sessions.reopen("b", &running(None));
    assert_eq!(plain.conversation.settled(), None);
}

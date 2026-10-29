use super::*;

#[test]
fn slugify_produces_file_name_safe_kebab_case() {
    assert_eq!(slugify("User Login / SSO!"), "user-login-sso");
    assert_eq!(slugify("../../etc"), "etc");
    assert_eq!(slugify("???"), "unnamed");
}

#[test]
fn unique_slug_suffixes_collisions() {
    let taken = ["login", "login-2"];
    assert_eq!(unique_slug("Login", taken.into_iter()), "login-3");
    assert_eq!(unique_slug("logout", taken.into_iter()), "logout");
}

#[test]
fn an_old_features_artifact_with_a_sub_domain_loads_and_is_saved_without_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = Artifact::Features.path(dir.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        "- slug: pay\n  domain_slug: billing\n  sub_domain_slug: payment\n  name: Pay\n  description: Pays.\n",
    )
    .unwrap();

    let features = load_features(dir.path()).expect("an old features.yaml still loads");
    assert_eq!(features[0].domain_slug, "billing");
    save_features(dir.path(), &features).unwrap();
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(!saved.contains("sub_domain"), "{saved}");
}

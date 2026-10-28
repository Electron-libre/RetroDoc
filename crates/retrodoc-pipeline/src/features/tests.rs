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

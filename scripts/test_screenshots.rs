#!/usr/bin/env rust-script
//! Behavior test for `screenshots.rs pages`: the pages it shoots, found from the links of the pages it reads. No
//! browser nor server needed. Usage: `just test-skill-scripts`.

use std::fs;
use std::path::Path;
use std::process::Command;

const HOME: &str = r#"<a href="/glossary">Glossary</a>
<a href="/domains/order-routing">Order routing</a>
<a href="/domains/billing">Billing</a>
<a href="/map/order-routing#top">Map</a>"#;
const FEATURE: &str = r#"<a href="/domains/order-routing/dispatch">Dispatch</a>
<a href="/domains/order-routing/dispatch/assign-a-courier">Assign</a>
<a href="/code/lib/router.rb?line=3&amp;to=9#L3">lib/router.rb</a>
<a href="https://example.com/domains/x">outside</a>"#;

fn main() {
    let dir = Path::new(file!()).parent().unwrap();
    let script = fs::canonicalize(dir.join("screenshots.rs")).unwrap();
    let tmp = std::env::temp_dir().join(format!("screenshots-test-{}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();
    fs::write(tmp.join("home.html"), HOME).unwrap();
    fs::write(tmp.join("feature.html"), FEATURE).unwrap();

    let out = Command::new("rust-script")
        .arg(&script)
        .args(["pages", "home.html", "feature.html"])
        .current_dir(&tmp)
        .output()
        .expect("rust-script must be installed");
    assert!(out.status.success(), "pages failed: {}", String::from_utf8_lossy(&out.stderr));
    let lines: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect();

    // The fixed pages come first, the home page among them.
    assert_eq!(lines[0], "home /");
    assert!(lines.contains(&"ask /ask".to_string()));
    // The first page of each kind, in the order of the pages read; anchors cut, entities decoded.
    for line in [
        "domain /domains/order-routing",
        "feature /domains/order-routing/dispatch",
        "use-case /domains/order-routing/dispatch/assign-a-courier",
        "domain-map /map/order-routing",
        "code /code/lib/router.rb?line=3&to=9",
        "search /search?q=order",
    ] {
        assert!(lines.contains(&line.to_string()), "missing `{line}` in {lines:?}");
    }
    assert!(!lines.iter().any(|l| l.contains("billing")), "a second domain is shot: {lines:?}");
    assert!(!lines.iter().any(|l| l.contains("example.com")), "an outside link is shot: {lines:?}");
    // A kind no page links to is left out, not shot empty.
    assert!(!lines.iter().any(|l| l.starts_with("collected-doc")), "{lines:?}");

    fs::remove_dir_all(&tmp).unwrap();
    println!("ok screenshots pages");
}

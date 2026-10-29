use super::*;
use crate::entry_points::{EntryFile, EntryKind, EntryPoint};

fn inventory(files: &[(&str, &[&str])]) -> EntryPoints {
    EntryPoints {
        files: files
            .iter()
            .map(|(path, names)| {
                (
                    PathBuf::from(path),
                    EntryFile {
                        content_hash: String::new(),
                        entry_points: names
                            .iter()
                            .map(|name| EntryPoint {
                                kind: EntryKind::HttpRoute,
                                name: (*name).to_string(),
                                verb: String::new(),
                                resource: String::new(),
                                description: String::new(),
                                outputs: Vec::new(),
                            })
                            .collect(),
                    },
                )
            })
            .collect(),
    }
}

fn kept(entries: &EntryPoints, file: &str) -> Vec<String> {
    entries.files[Path::new(file)]
        .entry_points
        .iter()
        .map(|e| e.name.clone())
        .collect()
}

/// A routes file declaring what three controllers handle, plus what no controller gives.
fn rails_app(routes: &[&str]) -> EntryPoints {
    inventory(&[
        ("config/routes.rb", routes),
        (
            "app/controllers/signup_controller.rb",
            &["GET /signup", "POST /signup", "GET /signup/invite"],
        ),
        (
            "app/controllers/invitations_controller.rb",
            &["GET /invitations/request", "POST /invitations"],
        ),
        (
            "app/controllers/stories_controller.rb",
            &[
                "POST /stories",
                "GET /stories/:id",
                "POST /stories/:id/upvote",
                "POST /stories/:id/unvote",
            ],
        ),
    ])
}

fn path(text: &str) -> RoutePath {
    paths_of(text).remove(0)
}

// spec: RFD-1 (internal: the filter runs inside `generate`, whose run through the binary needs an LLM)
#[test]
fn the_paths_of_a_name_drop_options_extensions_and_read_every_placeholder_alike() {
    assert_eq!(
        paths_of("GET /signup, POST /signup, /signup/invite and related"),
        vec![path("/signup"), path("/signup"), path("/signup/invite")]
    );
    assert_eq!(paths_of("GET /s/:id/(:title)"), vec![path("/s/:x")]);
    assert_eq!(paths_of("GET /tags.json"), vec![path("/tags")]);
    assert_eq!(
        paths_of("GET /api/bookmarks/"),
        vec![path("/api/bookmarks")]
    );
    for placeholder in [
        ":id", "{id}", "<id>", "<int:id>", "[id]", "*rest", ":id.png",
    ] {
        assert_eq!(
            paths_of(&format!("GET /users/{placeholder}")),
            vec![path("/users/:user")],
            "{placeholder}"
        );
    }
    assert_eq!(
        paths_of("MissionControl::Jobs engine"),
        Vec::<RoutePath>::new()
    );
    assert_eq!(paths_of("GET /"), vec![RoutePath::default()]);
}

// spec: RFD-2, RFD-8 (internal: the filter runs inside `generate`, whose run through the binary needs an LLM)
#[test]
fn a_short_path_after_a_longer_one_is_read_as_its_sibling() {
    let (kept_entries, left_out) = rails_app(&[
        "POST /stories/:id/upvote, /unvote",
        "GET /signup",
        "POST /invitations",
    ])
    .without_routes_declared_elsewhere();
    assert_eq!(
        left_out,
        [
            "config/routes.rb#POST /stories/:id/upvote, /unvote",
            "config/routes.rb#GET /signup",
            "config/routes.rb#POST /invitations",
        ]
    );
    assert_eq!(
        kept(&kept_entries, "config/routes.rb"),
        Vec::<String>::new()
    );
}

// spec: RFD-3 (internal: the filter runs inside `generate`, whose run through the binary needs an LLM)
#[test]
fn a_path_covers_the_longer_paths_it_starts_and_the_root_only_itself() {
    assert!(path("/invitations").covered_by(&path("/invitations/request")));
    assert!(path("/stories/:id").covered_by(&path("/stories/{story}")));
    assert!(!path("/invitations/request").covered_by(&path("/invitations")));
    assert!(!path("/").covered_by(&path("/signup")));
    assert!(path("/").covered_by(&path("/")));
}

// spec: RFD-4, RFD-5 (internal: the filter runs inside `generate`, whose run through the binary needs an LLM)
#[test]
fn a_routes_file_loses_the_routes_the_controllers_give() {
    let (kept_entries, left_out) = rails_app(&[
        "GET /signup, POST /signup, /signup/invite",
        "/invitations (resources)",
        "/stories (resources)",
        "GET /u, /u/:username",
    ])
    .without_routes_declared_elsewhere();
    assert_eq!(
        kept(&kept_entries, "config/routes.rb"),
        ["GET /u, /u/:username"]
    );
    assert_eq!(left_out.len(), 3);
    for controller in ["signup", "invitations", "stories"] {
        let file = format!("app/controllers/{controller}_controller.rb");
        assert_eq!(
            kept(&kept_entries, &file).len(),
            rails_app(&[]).files[Path::new(&file)].entry_points.len(),
            "{file}"
        );
    }
}

// spec: RFD-4 (internal: the filter runs inside `generate`, whose run through the binary needs an LLM)
#[test]
fn a_file_is_a_routing_file_only_with_three_other_files_and_half_its_routes_covered() {
    // Two other files only: not a routing file, even with every route covered.
    let two = inventory(&[
        ("home_controller.rb", &["GET /", "GET /t/:tag"]),
        ("config/routes.rb", &["GET /", "GET /t/:tag"]),
        ("tags_controller.rb", &["GET /t/:tag"]),
    ]);
    let (kept_entries, _) = two.without_routes_declared_elsewhere();
    assert_eq!(kept(&kept_entries, "home_controller.rb").len(), 2);

    // Three other files, but most of its routes are its own: not a routing file either.
    let (_, left_out) = rails_app(&[
        "GET /signup",
        "/invitations",
        "/stories",
        "GET /feed",
        "GET /rss",
        "GET /hottest",
        "GET /page/:page",
    ])
    .without_routes_declared_elsewhere();
    assert!(left_out.is_empty(), "{left_out:?}");
}

// spec: RFD-5 (internal: the filter runs inside `generate`, whose run through the binary needs an LLM)
#[test]
fn two_routing_files_do_not_take_routes_from_each_other() {
    let routes = ["GET /signup", "/invitations", "/stories/:id"];
    let mut entries = rails_app(&routes);
    entries.files.insert(
        PathBuf::from("config/routes/legacy.rb"),
        rails_app(&routes).files[Path::new("config/routes.rb")].clone(),
    );
    // Only the routes a controller gives go; each copy is covered by a controller.
    let (kept_entries, left_out) = entries.without_routes_declared_elsewhere();
    assert_eq!(left_out.len(), 6);
    assert_eq!(
        kept(&kept_entries, "config/routes.rb"),
        Vec::<String>::new()
    );

    // A route only the other routing file gives stays in both.
    let (kept_entries, left_out) = inventory(&[
        (
            "config/routes.rb",
            &["GET /signup", "/invitations", "GET /feed"],
        ),
        (
            "config/routes/legacy.rb",
            &["GET /signup", "/invitations", "GET /feed"],
        ),
        ("signup_controller.rb", &["GET /signup"]),
        ("invitations_controller.rb", &["POST /invitations"]),
    ])
    .without_routes_declared_elsewhere();
    assert_eq!(left_out.len(), 4, "{left_out:?}");
    assert_eq!(kept(&kept_entries, "config/routes.rb"), ["GET /feed"]);
    assert_eq!(
        kept(&kept_entries, "config/routes/legacy.rb"),
        ["GET /feed"]
    );
}

// spec: RFD-6 (internal: the filter runs inside `generate`, whose run through the binary needs an LLM)
#[test]
fn view_sets_and_routes_no_other_file_gives_are_kept() {
    let linkding = inventory(&[
        (
            "bookmarks/api/routes.py",
            &["GET /api/bookmarks/", "POST /api/bookmarks/:id/archive/"],
        ),
        (
            "bookmarks/views/bookmarks.py",
            &["GET /bookmarks/", "GET /bookmarks/:id/"],
        ),
        ("bookmarks/views/settings.py", &["GET /settings/"]),
        ("bookmarks/views/auth.py", &["GET /login/"]),
    ]);
    let (kept_entries, left_out) = linkding.clone().without_routes_declared_elsewhere();
    assert_eq!(left_out, Vec::<String>::new());
    assert_eq!(names(&kept_entries), names(&linkding));

    let mut with_other = rails_app(&[
        "GET /signup",
        "/invitations",
        "/stories",
        "GET /signup/invite",
    ]);
    let routes = with_other
        .files
        .get_mut(Path::new("config/routes.rb"))
        .unwrap();
    routes.entry_points[0].kind = EntryKind::Other;
    routes.entry_points.push(EntryPoint {
        name: "MissionControl::Jobs engine".to_string(),
        ..routes.entry_points[1].clone()
    });
    let (kept_entries, _) = with_other.without_routes_declared_elsewhere();
    assert_eq!(
        kept(&kept_entries, "config/routes.rb"),
        ["GET /signup", "MissionControl::Jobs engine"]
    );
}

fn names(entries: &EntryPoints) -> Vec<String> {
    entries.iter().map(|(_, e)| e.name.clone()).collect()
}

// spec: RFD-7 (internal: the filter runs inside `generate`, whose run through the binary needs an LLM)
#[test]
fn the_saved_inventory_keeps_every_entry() {
    let dir = tempfile::tempdir().unwrap();
    let entries = rails_app(&["GET /signup", "/invitations", "/stories"]);
    entries.save(dir.path()).unwrap();
    let (kept_entries, left_out) = EntryPoints::load(dir.path())
        .unwrap()
        .without_routes_declared_elsewhere();
    assert_eq!(left_out.len(), 3);
    assert_eq!(kept_entries.iter().count(), 9);
    let saved = EntryPoints::load(dir.path()).unwrap();
    assert_eq!(names(&saved), names(&entries));
}

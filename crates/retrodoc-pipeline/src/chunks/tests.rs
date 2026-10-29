use std::fmt::Write as _;

use super::*;

#[test]
fn short_content_is_one_chunk_and_long_content_loses_nothing() {
    assert_eq!(
        split_chunks("def a; end\n", 100, None),
        vec!["def a; end\n"]
    );
    assert_eq!(split_chunks("", 100, None), Vec::<String>::new());

    let one_line = "y".repeat(25);
    let chunks = split_chunks(&one_line, 10, None);
    assert_eq!(chunks.concat(), one_line);
    assert_eq!(chunks.len(), 3);
}

#[test]
fn without_a_boundary_rule_it_cuts_after_a_blank_line() {
    let content = "alpha\nbeta\n\n".repeat(10);
    let chunks = split_chunks(&content, 40, None);
    assert!(chunks.len() > 1);
    assert_eq!(chunks.concat(), content);
    assert!(chunks.iter().all(|c| c.starts_with("alpha")));
}

#[test]
fn a_boundary_rule_cuts_before_the_unit_even_without_blank_lines() {
    // Brace language, no blank line between functions.
    let content = "func a() {\n  work()\n}\n".repeat(10);
    let boundary = Regex::new(r"^func ").unwrap();
    let chunks = split_chunks(&content, 70, Some(&Boundary::Regex(&boundary)));
    assert!(chunks.len() > 1);
    assert_eq!(chunks.concat(), content);
    assert!(chunks.iter().all(|c| c.starts_with("func a")));
}

#[test]
fn a_parsed_file_is_cut_before_its_definitions_with_the_attributes_above() {
    // No regex rule at all: the parser finds the functions, and the attribute and
    // comment above each stay with it.
    let content = "/// Adds.\n#[inline]\npub fn add(a: u32, b: u32) -> u32 {\n    let sum = a + b;\n    sum\n}\n".repeat(8);
    let splitter = Splitter::new(&[]);
    let chunks = splitter.file_chunks(Path::new("src/lib.rs"), &content, 180, "test");
    assert!(chunks.len() > 1);
    let body: Vec<String> = chunks
        .iter()
        .map(|c| c.split_once('\n').unwrap().1.to_string())
        .collect();
    assert_eq!(body.concat(), content);
    assert!(body
        .iter()
        .all(|c| c.starts_with("/// Adds.\n#[inline]\npub fn add")));
}

#[test]
fn an_at_sign_above_a_definition_is_a_decorator_except_in_ruby() {
    let python = "import os\n@cache\ndef f():\n    return 1\n";
    assert_eq!(
        parsed_starts(Path::new("a.py"), python).unwrap(),
        BTreeSet::from([2])
    );
    let ruby = "class A\n  @count = 0\n  def f\n    1\n  end\nend\n";
    let starts = parsed_starts(Path::new("a.rb"), ruby).unwrap();
    assert!(starts.contains(&3) && !starts.contains(&2), "{starts:?}");
}

#[test]
fn a_ruby_file_is_cut_between_its_methods() {
    let content = "class Report\n".to_string()
        + &"  def run\n    work\n    more_work\n  end\n".repeat(10)
        + "end\n";
    let splitter = Splitter::new(&[]);
    let chunks = splitter.file_chunks(Path::new("app/report.rb"), &content, 120, "test");
    assert!(chunks.len() > 2);
    assert!(chunks[1..]
        .iter()
        .all(|c| c.split_once('\n').unwrap().1.starts_with("  def run")));
}

#[test]
fn a_parsed_file_without_definition_falls_back_to_the_rule_then_blank_lines() {
    let content = "puts 1\n\nputs 2\n\n".repeat(20);
    let splitter = Splitter::new(&[]);
    let chunks = splitter.file_chunks(Path::new("script.rb"), &content, 60, "test");
    assert!(chunks.len() > 1);
    let body: Vec<&str> = chunks
        .iter()
        .map(|c| c.split_once('\n').unwrap().1)
        .collect();
    assert_eq!(body.concat(), content);
    // Cut after a blank line, as without any parse.
    assert!(body[..body.len() - 1].iter().all(|c| c.ends_with("\n\n")));
}

#[test]
fn the_rule_is_picked_by_extension_and_a_bad_regex_is_ignored() {
    let splitter = Splitter::new(&[
        ChunkBoundary {
            extensions: vec!["(".to_string()],
            pattern: "(".to_string(),
        },
        ChunkBoundary {
            extensions: vec![".GO".to_string()],
            pattern: "^func ".to_string(),
        },
    ]);
    assert!(splitter.boundary_for(Path::new("a/main.go"), "").is_some());
    assert!(splitter.boundary_for(Path::new("a/main.rb"), "").is_none());
    assert!(splitter.boundary_for(Path::new("Makefile"), "").is_none());
}

#[test]
fn long_files_are_marked_with_their_parts_and_capped() {
    let splitter = Splitter::default();
    let content = "x\n".repeat(10 * 5);
    let chunks = splitter.file_chunks(Path::new("a.rb"), &content, 10, "test");
    assert_eq!(chunks.len(), MAX_CHUNKS_PER_FILE);
    assert!(chunks[0].starts_with("(part 1/8)"));
}

/// 40 methods of 11 lines; `target_a` is the 30th (line 321), `target_b`
/// the 12th (line 123).
fn controller() -> String {
    let mut content = String::from("class C\n");
    for i in 1..=40 {
        let name = match i {
            30 => "target_a".to_string(),
            12 => "target_b".to_string(),
            _ => format!("action_{i}"),
        };
        let _ = writeln!(content, "  def {name}");
        for step in 1..=8 {
            let _ = writeln!(content, "    work_{i}_{step}");
        }
        content.push_str("  end\n\n");
    }
    content.push_str("end\n");
    content
}

fn ruby() -> Splitter {
    Splitter::new(&[ChunkBoundary {
        extensions: vec!["rb".to_string()],
        pattern: r"^\s*def\s".to_string(),
    }])
}

#[test]
fn a_file_that_fits_is_given_whole_and_numbered() {
    let out = ruby().excerpt(Path::new("a.rb"), "x\ny\n", 1_000, &Focus::default());
    assert_eq!(out, "   1 | x\n   2 | y\n");
}

#[test]
fn a_long_file_shows_its_header_and_the_chunks_naming_the_focus() {
    let content = controller();
    let focus = Focus {
        terms: vec!["target_a".to_string()],
        ..Focus::default()
    };

    let out = ruby().excerpt(Path::new("c.rb"), &content, 4_000, &focus);

    assert!(out.len() <= 4_200, "{}", out.len());
    assert!(out.contains("| class C"), "header kept");
    assert!(out.contains("def target_a"), "the focused method is shown");
    assert!(!out.contains("def target_b"));
    let line = out.lines().find(|l| l.contains("def target_a")).unwrap();
    assert!(line.trim_start().starts_with("321 |"), "{line}");
    assert!(out.contains("omitted)"));
}

#[test]
fn cited_lines_outweigh_names_and_no_focus_keeps_the_beginning() {
    let content = controller();
    let cited = Focus {
        terms: vec!["target_a".to_string()],
        lines: vec![(123, 125)],
        ..Focus::default()
    };
    let out = ruby().excerpt(Path::new("c.rb"), &content, 4_000, &cited);
    assert!(out.contains("def target_b"), "{out}");

    let head = ruby().excerpt(Path::new("c.rb"), &content, 4_000, &Focus::default());
    assert!(head.contains("def action_1\n"));
    assert!(!head.contains("def target_a"));
    assert!(head.trim_end().ends_with("omitted)"));
}

#[test]
fn a_copied_part_marker_is_stripped_from_a_cited_path() {
    assert_eq!(
        strip_part_marker("config/routes.rb (part 1/2)"),
        "config/routes.rb"
    );
    assert_eq!(strip_part_marker(" a/b.rb "), "a/b.rb");
    assert_eq!(
        strip_part_marker("--- bookmarks/views/auth.py ---"),
        "bookmarks/views/auth.py"
    );
    assert_eq!(strip_part_marker("-a/b-c.rb"), "-a/b-c.rb");
    assert_eq!(strip_part_marker("a/(part)/b.rb"), "a/(part)/b.rb");
    assert_eq!(strip_part_marker("a/b(1).rb"), "a/b(1).rb");
}

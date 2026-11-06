#!/usr/bin/env rust-script
//! Screenshots of every page of a running `retrodoc ui`, to compare the look of the web interface before and after
//! a change (`docs/dev/web-ui.md`).
//!
//! - `screenshots.rs shoot <base-url> <label>`: finds one page of each kind from the links of the home page and of
//!   the pages it leads to, then shoots each at a desktop and a narrow width (1440 and 500 px) with headless Chrome into
//!   `<runs directory>/screenshots/<label>/<kind>-<width>.png` (`$RETRODOC_RUNS_DIR`, else `~/retrodoc-runs`).
//! - `screenshots.rs pages <file.html>...`: only the pages it would shoot, found in the given HTML, one
//!   `<kind> <path>` line each.
//!
//! Usage: `just screenshots <base-url> <label>`, with `retrodoc ui --ask` serving the docs at `<base-url>`.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::Command;

/// The widths shot, with the height of the window: tall enough for the forms at the foot of most pages. The narrow
/// one is the smallest window headless Chrome draws (it widens a narrower one to 500 px), under the 48rem of the
/// narrow layout of `style.css`.
const WIDTHS: [(u32, u32); 2] = [(1440, 4000), (500, 6000)];
/// The pages every site has, at a fixed path.
const FIXED: [(&str, &str); 11] = [
    ("home", "/"),
    ("files", "/files"),
    ("glossary", "/glossary"),
    ("rules", "/rules"),
    ("actors", "/actors"),
    ("debt", "/debt"),
    ("audit", "/audit"),
    ("history", "/history"),
    ("map", "/map"),
    ("ask", "/ask"),
    ("not-found", "/no-such-page"),
];
/// The pages found from the links, in the order they are looked for.
const FOUND: [&str; 6] = ["domain", "feature", "use-case", "domain-map", "code", "collected-doc"];
/// At most this many pages are read to find the pages of `FOUND`: the home page, the fixed pages, then the pages
/// found.
const MAX_READS: usize = 20;

/// The `href` values of `html`, entities `&amp;` decoded and anchors cut.
fn hrefs(html: &str) -> Vec<String> {
    html.split("href=\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .map(|href| href.replace("&amp;", "&"))
        .map(|href| href.split('#').next().unwrap_or_default().to_string())
        .filter(|href| href.starts_with('/'))
        .collect()
}

/// The kind of page of `FOUND` that `path` is, if any.
fn kind(path: &str) -> Option<&'static str> {
    let segments: Vec<&str> = path.trim_start_matches('/').split('/').filter(|s| !s.is_empty()).collect();
    match segments.as_slice() {
        ["domains", _] => Some("domain"),
        ["domains", _, _] => Some("feature"),
        ["domains", _, _, _] => Some("use-case"),
        ["map", _] => Some("domain-map"),
        ["code", _, ..] => Some("code"),
        ["docs", _, ..] => Some("collected-doc"),
        _ => None,
    }
}

/// Records in `found` the first page of each kind among the links of `html`.
fn collect(html: &str, found: &mut Vec<(&'static str, String)>) {
    for href in hrefs(html) {
        if let Some(kind) = kind(&href) {
            if !found.iter().any(|(k, _)| *k == kind) {
                found.push((kind, href));
            }
        }
    }
}

/// The search page, on the first word of the first domain's slug (a word the docs hold).
fn search(found: &[(&'static str, String)]) -> Option<String> {
    let domain = found.iter().find(|(k, _)| *k == "domain")?;
    let slug = domain.1.rsplit('/').next()?;
    let word = slug.split('-').next().filter(|w| !w.is_empty())?;
    Some(format!("/search?q={word}"))
}

/// Every page to shoot: the fixed ones, then the found ones in the order of `FOUND`, then the search.
fn pages(found: &[(&'static str, String)]) -> Vec<(String, String)> {
    let mut pages: Vec<(String, String)> = FIXED.iter().map(|(k, p)| ((*k).to_string(), (*p).to_string())).collect();
    for kind in FOUND {
        if let Some((_, path)) = found.iter().find(|(k, _)| *k == kind) {
            pages.push((kind.to_string(), path.clone()));
        }
    }
    if let Some(path) = search(found) {
        pages.push(("search".to_string(), path));
    }
    pages
}

/// The body of `GET <path>` on `host` (HTTP/1.0: the server answers without chunks and closes); an answer other
/// than 200 is an error, so a wrong `--path` does not pass for a site.
fn get(host: &str, path: &str) -> Result<String, String> {
    let mut stream = TcpStream::connect(host).map_err(|e| format!("connect to {host}: {e}"))?;
    write!(stream, "GET {path} HTTP/1.0\r\nHost: {host}\r\n\r\n").map_err(|e| e.to_string())?;
    let mut answer = String::new();
    stream.read_to_string(&mut answer).map_err(|e| e.to_string())?;
    let (head, body) = answer.split_once("\r\n\r\n").ok_or_else(|| format!("GET {path}: no HTTP answer"))?;
    let status = head.lines().next().unwrap_or_default();
    if status.split_whitespace().nth(1) != Some("200") {
        return Err(format!("GET {path}: {status}"));
    }
    Ok(body.to_string())
}

/// Reads the home page, the fixed pages, then the pages found so far, until every kind of `FOUND` is found or
/// `MAX_READS` pages were read.
fn discover(host: &str) -> Result<Vec<(&'static str, String)>, String> {
    let mut found: Vec<(&'static str, String)> = Vec::new();
    let fixed = FIXED.iter().filter(|(k, _)| *k != "not-found").map(|(_, p)| (*p).to_string());
    let mut queue: Vec<String> = fixed.collect();
    let mut read = 0;
    while read < queue.len() && read < MAX_READS && found.len() < FOUND.len() {
        let before = found.len();
        collect(&get(host, &queue[read])?, &mut found);
        queue.extend(found[before..].iter().map(|(_, path)| path.clone()));
        read += 1;
    }
    Ok(found)
}

fn runs_dir() -> PathBuf {
    std::env::var_os("RETRODOC_RUNS_DIR").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("HOME").expect("HOME is set")).join("retrodoc-runs")
    })
}

fn die(message: &str) -> ! {
    eprintln!("screenshots: {message}");
    std::process::exit(1);
}

fn shoot(base: &str, label: &str) {
    let base = base.trim_end_matches('/');
    let host = base.strip_prefix("http://").unwrap_or_else(|| die("the base URL must start with http://"));
    let found = discover(host).unwrap_or_else(|e| die(&e));
    for kind in FOUND {
        if !found.iter().any(|(k, _)| *k == kind) {
            eprintln!("screenshots: no page of kind {kind} found, not shot");
        }
    }
    let dir = runs_dir().join("screenshots").join(label);
    fs::create_dir_all(&dir).unwrap_or_else(|e| die(&format!("create {}: {e}", dir.display())));
    // The shots of an earlier run under the same label go first: none of them may pass for one of this run, also
    // of a kind this run did not find.
    for entry in fs::read_dir(&dir).unwrap_or_else(|e| die(&format!("read {}: {e}", dir.display()))) {
        let path = entry.unwrap_or_else(|e| die(&e.to_string())).path();
        if path.extension().is_some_and(|e| e == "png") {
            fs::remove_file(&path).unwrap_or_else(|e| die(&format!("remove {}: {e}", path.display())));
        }
    }
    for (kind, path) in pages(&found) {
        for (width, height) in WIDTHS {
            let file = dir.join(format!("{kind}-{width}.png"));
            let status = Command::new("google-chrome-stable")
                .args(["--headless=new", "--disable-gpu", "--hide-scrollbars", "--virtual-time-budget=3000"])
                .arg(format!("--window-size={width},{height}"))
                .arg(format!("--screenshot={}", file.display()))
                .arg(format!("{base}{path}"))
                .output()
                .unwrap_or_else(|e| die(&format!("run google-chrome-stable: {e}")));
            if !status.status.success() || !file.exists() {
                die(&format!("chrome failed on {path}: {}", String::from_utf8_lossy(&status.stderr)));
            }
        }
        println!("{kind} {path}");
    }
    println!("written to {}", dir.display());
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("shoot") if args.len() == 3 => shoot(&args[1], &args[2]),
        Some("pages") if args.len() > 1 => {
            let mut found = Vec::new();
            for file in &args[1..] {
                let html = fs::read_to_string(file).unwrap_or_else(|e| die(&format!("read {file}: {e}")));
                collect(&html, &mut found);
            }
            for (kind, path) in pages(&found) {
                println!("{kind} {path}");
            }
        }
        _ => die("usage: screenshots.rs shoot <base-url> <label> | pages <file.html>..."),
    }
}

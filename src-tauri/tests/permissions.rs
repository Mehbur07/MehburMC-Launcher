//! Every command in `generate_handler!` must be listed in `build.rs` and
//! granted in `capabilities/default.json`; otherwise the webview gets
//! "Command not found" at runtime.

use std::collections::BTreeSet;

fn read(rel: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)).unwrap()
}

#[test]
fn handlers_are_declared_and_granted() {
    let lib = read("src/lib.rs");
    let start = lib.find("generate_handler![").unwrap();
    let end = start + lib[start..].find(']').unwrap();
    let handlers: BTreeSet<String> = lib[start + "generate_handler![".len()..end]
        .split(',')
        .map(|s| s.trim().rsplit("::").next().unwrap().to_owned())
        .filter(|s| !s.is_empty())
        .collect();

    let build = read("build.rs");
    let caps = read("capabilities/default.json");
    for h in &handlers {
        assert!(
            build.contains(&format!("\"{h}\"")),
            "{h} missing in build.rs COMMANDS"
        );
        let perm = format!("\"allow-{}\"", h.replace('_', "-"));
        assert!(
            caps.contains(&perm),
            "{perm} missing in capabilities/default.json"
        );
    }
    assert!(
        handlers.len() > 50,
        "parsed only {} handlers",
        handlers.len()
    );
}

//! The wall check is a served tool a consumer can find and run.
//!
//! Written from `fact:root-cause-the-wall-check-was-generalised-in-code-and-
//! never-in-reach-and-the-checkout-cannot-feel-it`: the analysis existed as a
//! script in this checkout, `find_tools` could not return it, no skill named
//! it, and a consumer re-wrote it by hand. The first test is the instance pin
//! — it FAILED before the tool was served.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value, json};

async fn top5(s: &ReflowService, query: &str) -> Vec<String> {
    let v: Value = s
        .find_tools(Parameters(
            serde_json::from_value(json!({"query": query, "limit": 5})).unwrap(),
        ))
        .await
        .expect("find_tools")
        .structured_content
        .expect("structured");
    v["items"]
        .as_array()
        .expect("items")
        .iter()
        .filter_map(|i| i["tool"].as_str().map(String::from))
        .collect()
}

fn text_of(r: rmcp::model::CallToolResult) -> String {
    r.content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The question the consumer's agent asked, in a user's words, now finds the
/// tool. Observed failing before `wall_check` was served.
#[tokio::test]
async fn the_coupling_question_in_a_users_words_finds_the_wall_check() {
    let s = ReflowService::in_memory().expect("service");
    let top = top5(
        &s,
        "does my declared decomposition match the real coupling — walk the imports across the boundaries I declared",
    )
    .await;
    assert!(top.contains(&"wall_check".to_string()), "top5 = {top:?}");
}

/// A real run: two components, two registered files, one import the design
/// never declared — the report names it, and the root it ran from.
#[tokio::test]
async fn it_runs_over_the_registered_artifacts_and_reports_the_undeclared_coupling() {
    let root = std::env::temp_dir().join(format!("reflow2-wall-check-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("pkg")).unwrap();
    std::fs::write(root.join("pkg/__init__.py"), "").unwrap();
    // `import pkg.b`, not `from pkg import b`: the reader resolves the latter to
    // the package (`pkg/__init__.py`), which this fixture deliberately leaves
    // unregistered so the "never heard of" count has something to say.
    std::fs::write(root.join("pkg/a.py"), "import pkg.b\n").unwrap();
    std::fs::write(root.join("pkg/b.py"), "x = 1\n").unwrap();

    let s = ReflowService::in_memory().expect("service");
    for (id, name) in [("cmp:a", "a"), ("cmp:b", "b")] {
        s.add_component(Parameters(
            serde_json::from_value(json!({"id": id, "name": name, "description": name})).unwrap(),
        ))
        .await
        .expect("component");
    }
    for (art, loc, cmp) in [
        ("art:a", "pkg/a.py", "cmp:a"),
        ("art:b", "pkg/b.py", "cmp:b"),
    ] {
        s.link_artifact(Parameters(
            serde_json::from_value(json!({
                "artifact_id": art, "name": loc, "location": loc, "artifact_type": "code",
                "to_type": "Component", "to_id": cmp
            }))
            .unwrap(),
        ))
        .await
        .expect("link");
    }

    let out = s
        .wall_check(Parameters(
            serde_json::from_value(json!({"root": root.to_string_lossy()})).unwrap(),
        ))
        .await
        .expect("wall_check");
    let text = text_of(out);
    assert!(
        text.contains(&format!("root: {}", root.display())),
        "{text}"
    );
    assert!(text.contains("DO THE DECLARED WALLS HOLD"), "{text}");
    assert!(
        text.contains("1 coupling edge(s) found"),
        "the import was read: {text}"
    );
    assert!(
        text.contains("1 pair(s) the SOURCE has and the design does not"),
        "the undeclared coupling is reported as exactly that: {text}"
    );
    assert!(
        text.contains("SOURCE FILE(S) THE DESIGN HAS NEVER HEARD OF")
            && text.contains("pkg/__init__.py"),
        "the unregistered file is counted and named, not scored clean: {text}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// A design with nothing registered gets the script's own honest sentence,
/// not an empty report and not an error.
#[tokio::test]
async fn nothing_registered_is_said_not_scored_clean() {
    let s = ReflowService::in_memory().expect("service");
    let out = s
        .wall_check(Parameters(serde_json::from_value(json!({})).unwrap()))
        .await
        .expect("wall_check");
    let text = text_of(out);
    assert!(
        text.contains("0 component(s) in the design") || text.contains("COVERAGE"),
        "{text}"
    );
}

/// The bound is honoured and announced.
#[tokio::test]
async fn budget_chars_bounds_the_report_and_says_so() {
    let s = ReflowService::in_memory().expect("service");
    let out = s
        .wall_check(Parameters(
            serde_json::from_value(json!({"budget_chars": 200})).unwrap(),
        ))
        .await
        .expect("wall_check");
    let text = text_of(out);
    assert!(text.chars().count() <= 200, "{}", text.chars().count());
    assert!(text.contains("[bounded:"), "{text}");
}

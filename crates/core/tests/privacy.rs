//! The same cases as test/privacy.test.mjs, against the Rust core.
mod common;
use common::*;
use serde_json::json;
use threadkeep_core::access::preview_context;
use threadkeep_core::home::load_config;
use threadkeep_core::search::{ask, AskArgs};

#[test]
fn an_ai_never_receives_private_malformed_ignored_or_ungranted_content() {
    let w = world();
    let ai = w.connect(&[("Northwind", "write")]);
    for question in ["Dana salary confidential amount", "never closed hidden", "performance reviews notes", "Mira morning appointments", "canary"] {
        let r = ai.call("ask", json!({ "question": question, "limit": 8 }));
        assert!(r.error.is_none(), "{:?}", r.error);
        for c in CANARIES {
            assert!(!r.text.contains(c), "{c} leaked for \"{question}\"");
        }
        assert!(r.data["sources"].as_array().unwrap().iter().all(|s| s["space"] == "Northwind"));
    }
    for reference in ["Northwind::Finance/Salaries.md", "Northwind::Broken.md", "Northwind::HR/Reviews.md", "Mira::Health.md", "Northwind::../Mira/Health.md", "Northwind::.threadkeepignore"] {
        let r = ai.call("open", json!({ "ref": reference }));
        assert!(r.error.is_some(), "open {reference} should be refused");
        for c in CANARIES {
            assert!(!r.text.contains(c));
        }
    }
    let named = ai.call("ask", json!({ "question": "appointments", "space": "Mira" }));
    assert!(named.error.unwrap().contains("No space named \"Mira\" is available here"));
}

#[test]
fn the_owner_can_search_private_notes_but_ignored_folders_stay_out() {
    let w = world();
    let r = ask(&w.owner(), &AskArgs { question: "Dana confidential amount".into(), ..Default::default() }).unwrap();
    assert!(r.to_string().contains("COMPANY_PRIVATE_CANARY"));
    let r = ask(&w.owner(), &AskArgs { question: "performance reviews".into(), ..Default::default() }).unwrap();
    assert!(!r.to_string().contains("IGNORED_FOLDER_CANARY"));
}

#[test]
fn see_as_this_ai_matches_what_the_connection_gets() {
    let w = world();
    let ai = w.connect(&[("Northwind", "read")]);
    let ctx = preview_context(&load_config(&w.home).unwrap(), &w.home, &ai.id).unwrap();
    let preview = ask(&ctx, &AskArgs { question: "Dana confidential amount salary".into(), ..Default::default() }).unwrap();
    assert!(!preview.to_string().contains("COMPANY_PRIVATE_CANARY"));
    let real = ai.call("ask", json!({ "question": "Dana confidential amount salary" }));
    let refs = |v: &serde_json::Value| v["sources"].as_array().unwrap().iter().map(|s| s["ref"].clone()).collect::<Vec<_>>();
    assert_eq!(refs(&preview), refs(&real.data));
}

#[test]
fn no_key_a_wrong_key_or_a_revoked_key_gets_nothing() {
    let w = world();
    let ai = w.connect(&[("Northwind", "write")]);
    assert!(ai.call("ask", json!({ "question": "Riverbend contract" })).error.is_none());
    w.revoke(&ai);
    // No restart: the very next request is refused.
    let refused = ai.call("ask", json!({ "question": "Riverbend contract" }));
    assert!(refused.error.as_ref().unwrap().contains("not connected"));
    assert!(!refused.text.contains("Riverbend support contract"));
    let wrong = format!("tk_{}", "a".repeat(48));
    for token in [None, Some(""), Some(wrong.as_str())] {
        assert!(call_with_token(&w.home, token, "ask", json!({ "question": "Riverbend" })).error.is_some());
    }
}

#[test]
fn changing_access_takes_effect_on_the_next_request() {
    let w = world();
    let ai = w.connect(&[("Northwind", "read")]);
    let save = json!({ "statement": "Riverbend prefers email.", "source": "test" });
    assert!(ai.call("save", save.clone()).error.unwrap().contains("read-only"));
    w.set_level(&ai, "Northwind", "write");
    assert_eq!(ai.call("save", save).data["saved"], true);
}

#[test]
fn word_endings_match_and_found_describes_the_returned_passages() {
    let w = world();
    let ai = w.connect(&[("Northwind", "read")]);
    // "renew" must match "renews"; "contracts" must match "contract".
    let r = ai.call("ask", json!({ "question": "When do the Riverbend contracts renew?" })).data;
    assert_eq!(r["status"], "found");
    assert_eq!(r["missing"].as_array().unwrap().len(), 0);
    assert_eq!(r["sources"][0]["heading"], "Contract");
    // Nothing relevant at all is "none", with no sources.
    let none = ai.call("ask", json!({ "question": "zeppelin maintenance schedule" })).data;
    assert_eq!(none["status"], "none");
    assert_eq!(none["sources"].as_array().unwrap().len(), 0);
    // One matching word out of several is "partial", and says what was not covered.
    let partial = ai.call("ask", json!({ "question": "Riverbend zeppelin maintenance schedule" })).data;
    assert_eq!(partial["status"], "partial");
    assert!(partial["missing"].as_array().unwrap().iter().any(|m| m == "zeppelin"));
}

#[test]
fn status_reports_levels_and_what_each_write_will_do() {
    let w = world();
    let ai = w.connect(&[("Northwind", "propose"), ("Mira", "write")]);
    let s = ai.call("status", json!({})).data;
    assert_eq!(s["spaces"][0]["save"], "waits for owner approval");
    assert_eq!(s["spaces"][0]["checkpoint"], "stored at once");
    assert_eq!(s["spaces"][1]["sensitive"], true);
    assert_eq!(s["spaces"][1]["save"], "waits for owner approval");
    assert_eq!(s["spaces"][1]["checkpoint"], "waits for owner approval");
}

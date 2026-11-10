//! The same cases as test/memory.test.mjs, plus work threads, against the Rust core.
mod common;
use common::*;
use serde_json::json;
use std::fs;
use threadkeep_core::home::load_config;
use threadkeep_core::memory::{forget_memory, list_memories, retire_memory, save_memory, MemoryInput};
use threadkeep_core::pending::{approve_pending, list_pending, reject_pending};
use threadkeep_core::threads::list_threads;

#[test]
fn propose_level_waits_for_the_owner_and_writes_nothing_until_approved() {
    let w = world();
    let ai = w.connect(&[("Northwind", "propose")]);
    let r = ai.call("save", json!({ "statement": "Riverbend wants invoices on the first of the month.", "source": "client call, fictional" }));
    assert_eq!(r.data["saved"], false);
    assert_eq!(r.data["waiting_for_approval"], true);
    assert!(memory_files(&w.company).is_empty());
    let before = ai.call("ask", json!({ "question": "Riverbend invoices first of the month" }));
    assert!(!before.data["sources"].as_array().unwrap().iter().any(|s| s["kind"] == "memory"));
    assert_eq!(ai.call("status", json!({})).data["waiting_for_approval"], 1);

    let pending = list_pending(&w.home);
    assert_eq!(pending[0]["connection"]["name"], "Test AI");
    let done = approve_pending(&w.home, &load_config(&w.home).unwrap(), pending[0]["id"].as_str().unwrap()).unwrap();
    assert_eq!(done["result"]["saved"], true);
    assert_eq!(memory_files(&w.company).len(), 1);
    let found = ai.call("ask", json!({ "question": "When does Riverbend want invoices?" }));
    assert_eq!(found.data["sources"][0]["kind"], "memory");
    assert_eq!(found.data["sources"][0]["confidence"], "reported");
    assert!(list_pending(&w.home).is_empty());
}

#[test]
fn a_rejected_proposal_leaves_no_trace_in_the_space() {
    let w = world();
    let ai = w.connect(&[("Northwind", "propose")]);
    ai.call("save", json!({ "statement": "A wrong guess about Riverbend.", "source": "inference" }));
    let pending = list_pending(&w.home);
    reject_pending(&w.home, pending[0]["id"].as_str().unwrap()).unwrap();
    assert!(memory_files(&w.company).is_empty());
    assert!(list_pending(&w.home).is_empty());
}

#[test]
fn a_sensitive_space_needs_approval_even_at_write_level() {
    let w = world();
    let ai = w.connect(&[("Mira", "write")]);
    assert_eq!(ai.call("save", json!({ "statement": "Mira likes window seats.", "source": "said in chat" })).data["waiting_for_approval"], true);
    assert_eq!(ai.call("checkpoint", json!({ "title": "Trip plan", "state": "Flights compared", "next_action": "Book the morning flight" })).data["waiting_for_approval"], true);
    assert!(!w.personal.join("Threadkeep").exists());
    let cfg = load_config(&w.home).unwrap();
    for p in list_pending(&w.home) {
        approve_pending(&w.home, &cfg, p["id"].as_str().unwrap()).unwrap();
    }
    assert_eq!(list_memories(&w.space("Mira"), None).len(), 1);
    assert_eq!(list_threads(&w.space("Mira"), false).len(), 1);
}

#[test]
fn a_correction_replaces_the_old_memory_which_stays_as_history() {
    let w = world();
    let ai = w.connect(&[("Northwind", "write")]);
    let first = ai.call("save", json!({ "statement": "The Riverbend account lead is Dana Whitfield.", "source": "kickoff notes" })).data;
    let fixed = ai.call("save", json!({ "statement": "The Riverbend account lead is Priya Raman.", "source": "owner correction in chat", "replaces": first["id"] })).data;
    assert_eq!(fixed["replaced"], first["id"]);

    let now = ai.call("ask", json!({ "question": "Who is the Riverbend account lead?" })).data;
    let memories: Vec<_> = now["sources"].as_array().unwrap().iter().filter(|s| s["kind"] == "memory").collect();
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0]["id"], fixed["id"]);
    assert!(memories[0]["excerpt"].as_str().unwrap().contains("Priya Raman"));

    let past = ai.call("ask", json!({ "question": "Who is the Riverbend account lead?", "history": true })).data;
    let old = past["sources"].as_array().unwrap().iter().find(|s| s["id"] == first["id"]).expect("old record returned with history");
    assert_eq!(old["outdated"], true);
    assert_eq!(old["replaced_by"], fixed["id"]);

    // Correcting the already-replaced record is refused instead of forking history.
    assert!(ai.call("save", json!({ "statement": "Someone else.", "source": "x", "replaces": first["id"] })).error.unwrap().contains("superseded"));
}

#[test]
fn duplicates_are_skipped_secrets_refused_and_fields_checked() {
    let w = world();
    let ai = w.connect(&[("Northwind", "write")]);
    let a = ai.call("save", json!({ "statement": "Northwind closes its books on the 5th.", "source": "finance lead" })).data;
    let b = ai.call("save", json!({ "statement": "northwind closes its books on the 5th", "source": "again" })).data;
    assert_eq!(b["already_known"], a["id"]);
    for statement in ["The portal password is hunter2-swordfish", "Use key sk-proj-abcdefghijklmnopqrstuvwxyz123456", "Card 4111 1111 1111 1111 is on file"] {
        assert!(ai.call("save", json!({ "statement": statement, "source": "chat" })).error.unwrap().contains("does not store credentials"), "{statement}");
    }
    assert!(ai.call("save", json!({ "statement": "No source given." })).error.unwrap().contains("source"));
    assert!(ai.call("save", json!({ "statement": "Bad kind.", "source": "x", "kind": "rumour" })).error.unwrap().contains("kind must be"));
    assert_eq!(memory_files(&w.company).len(), 1);
}

#[test]
fn saving_needs_a_named_space_when_more_than_one_accepts_saves() {
    let w = world();
    let ai = w.connect(&[("Northwind", "write"), ("Mira", "propose")]);
    assert!(ai.call("save", json!({ "statement": "Ambiguous.", "source": "x" })).error.unwrap().contains("Say which space"));
    assert_eq!(ai.call("save", json!({ "statement": "Goes to the company.", "source": "x", "space": "Northwind" })).data["saved"], true);
}

#[test]
fn retire_hides_a_memory_and_forget_deletes_it() {
    let w = world();
    let ai = w.connect(&[("Northwind", "write")]);
    let m = ai.call("save", json!({ "statement": "Zephyr Labs is a prospect for the spring campaign.", "source": "sales note" })).data;
    let id = m["id"].as_str().unwrap();
    let space = w.space("Northwind");
    retire_memory(&space, id).unwrap();
    assert_eq!(ai.call("ask", json!({ "question": "Zephyr Labs prospect" })).data["status"], "none");
    assert_eq!(ai.call("ask", json!({ "question": "Zephyr Labs prospect", "history": true })).data["status"], "none");
    let r = forget_memory(&space, id).unwrap();
    assert!(memory_files(&w.company).is_empty());
    assert!(r["not_removed"].as_array().unwrap().iter().any(|x| x.as_str().unwrap().contains("Backups")));
}

#[test]
fn hand_edits_and_sync_conflict_copies_do_not_break_anything() {
    let w = world();
    let space = w.space("Northwind");
    let saved = save_memory(&space, &MemoryInput { statement: "The office is closed on Fridays.".into(), source: "owner".into(), ..Default::default() }).unwrap();
    let id = saved["id"].as_str().unwrap();
    let dir = w.company.join("Threadkeep").join("Memories");
    let raw = fs::read_to_string(dir.join(format!("{id}.md"))).unwrap();
    fs::write(dir.join(format!("{id} (conflicted copy).md")), &raw).unwrap();
    fs::write(dir.join("notes.txt"), "stray").unwrap();
    fs::write(dir.join(format!("{id}.md")), raw.replace("Fridays", "Mondays")).unwrap();
    let list = list_memories(&space, None);
    assert_eq!(list.len(), 1);
    assert!(list[0].statement.contains("Mondays"));
}

#[test]
fn one_ai_checkpoints_and_another_resumes() {
    let w = world();
    let first = w.connect(&[("Northwind", "propose")]);
    let second = w.connect(&[("Northwind", "read")]);
    let cp = first.call("checkpoint", json!({
        "title": "Renewal proposal", "goal": "Draft the Riverbend renewal proposal", "state": "Pricing table done; cover letter not started",
        "next_action": "Write the cover letter", "decisions": ["Offer a two-year term: client asked for price stability"], "sources": ["Northwind::Clients/Riverbend.md"],
    })).data;
    assert_eq!(cp["saved"], true);
    let thread = cp["thread"].as_str().unwrap().to_string();

    let resumed = second.call("resume", json!({})).data;
    assert_eq!(resumed["thread"], thread.as_str());
    assert_eq!(resumed["goal"], "Draft the Riverbend renewal proposal");
    assert_eq!(resumed["next_action"], "Write the cover letter");
    assert_eq!(resumed["decisions"][0], "Offer a two-year term: client asked for price stability");
    assert_eq!(resumed["open_questions"].as_array().unwrap().len(), 0);
    assert_eq!(resumed["last_checkpoint"]["by"], "Test AI");

    // A read-only connection cannot checkpoint.
    assert!(second.call("checkpoint", json!({ "thread": thread, "state": "x", "next_action": "y" })).error.is_some());

    // A later checkpoint becomes the thread's state; the goal carries forward; nothing was edited in place.
    std::thread::sleep(std::time::Duration::from_millis(5));
    first.call("checkpoint", json!({ "thread": thread, "state": "Cover letter drafted", "next_action": "Send for review" }));
    let again = second.call("resume", json!({ "thread": thread })).data;
    assert_eq!(again["state"], "Cover letter drafted");
    assert_eq!(again["goal"], "Draft the Riverbend renewal proposal");
    assert_eq!(again["checkpoints"], 2);
    assert_eq!(fs::read_dir(w.company.join("Threadkeep").join("Threads").join(&thread)).unwrap().count(), 2);

    // A second thread makes resume return a list; finishing one removes it from the list.
    let other = first.call("checkpoint", json!({ "title": "Onboarding checklist", "state": "Outline written", "next_action": "Fill in week two" })).data;
    assert_eq!(second.call("resume", json!({})).data["open_threads"].as_array().unwrap().len(), 2);
    first.call("checkpoint", json!({ "thread": other["thread"], "state": "Done", "next_action": "Nothing further", "status": "done" }));
    assert_eq!(second.call("resume", json!({})).data["thread"], thread.as_str());

    // Threads never leak across spaces.
    let outsider = w.connect(&[("Mira", "read")]);
    assert!(outsider.call("resume", json!({ "thread": thread })).error.is_some());
    assert_eq!(outsider.call("resume", json!({})).data["open_threads"].as_array().unwrap().len(), 0);
}

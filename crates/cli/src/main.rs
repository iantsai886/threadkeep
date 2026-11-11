//! Threadkeep command line. Everything the product does is reachable from here, and
//! `threadkeep mcp` is the program AI apps launch.

mod app;

use serde_json::{json, Map, Value};
use std::io::{BufRead, Write};
use threadkeep_core::access::connection_context;
use threadkeep_core::connectors::client_ids;
use threadkeep_core::home::{app_home, load_config, PRODUCT, VERSION};
use threadkeep_core::mcp;
use threadkeep_core::ops::Ops;

fn help() -> String {
    format!(
        "{PRODUCT} {VERSION} — your own memory and work threads, shared safely with your AI apps.

Set up
  threadkeep space add <folder> [--name N] [--kind personal|company|client] [--create] [--sensitive|--open]
  threadkeep space list | space set <name> [--kind K] [--sensitive|--open] | space remove <name>
  threadkeep connect <app> --space <name>[:read|propose|write] [--space ...] [--name N]
        apps: {apps}, custom
  threadkeep connections | access <id> --space <name>[:level] ... | disconnect <id|app>
  threadkeep status            what is set up and whether each AI app has really connected
  threadkeep doctor            find problems and how to fix them

Daily use
  threadkeep review | approve <proposal> | reject <proposal>
  threadkeep ask \"<question>\" [--space S] [--as <connection>] [--history]
  threadkeep open \"<ref>\" [--as <connection>]
  threadkeep remember \"<statement>\" --source \"<where from>\" [--space S] [--kind K] [--replaces <id>]
  threadkeep memories [--space S] [--status active|superseded|retired]
  threadkeep retire <id> [--space S] | forget <id> [--space S]
  threadkeep threads [--space S] [--all]

Safety net
  threadkeep backup <space> <folder> | verify <backup-folder> | restore <backup-folder> <new-folder>
  threadkeep backups [folder]   backups in a folder (default: where the last backup went)
  threadkeep activity [--n 30]  tool names and outcomes only, never content
  threadkeep overhead           size of what every AI conversation carries

Run
  threadkeep app [--port P] [--no-open]   local app (this computer only)
  threadkeep mcp                          what AI apps launch; not for people

Access levels: read = ask and resume; propose = may also suggest memories, which wait for your
approval; write = memories are stored at once. A sensitive space always needs your approval.
Settings folder: {home}  (override with THREADKEEP_HOME)",
        apps = client_ids(),
        home = app_home().display()
    )
}

struct Parsed {
    pos: Vec<String>,
    flags: Map<String, Value>,
    spaces: Vec<String>,
}

fn parse(args: &[String]) -> Parsed {
    let mut p = Parsed { pos: vec![], flags: Map::new(), spaces: vec![] };
    let mut i = 0;
    while i < args.len() {
        match args[i].strip_prefix("--") {
            None => p.pos.push(args[i].clone()),
            Some(key) => {
                let value = args.get(i + 1).filter(|v| !v.starts_with("--")).cloned();
                if value.is_some() {
                    i += 1;
                }
                match (key, value) {
                    ("space", Some(v)) => p.spaces.push(v),
                    (k, Some(v)) => {
                        p.flags.insert(k.replace('-', "_"), json!(v));
                    }
                    (k, None) => {
                        p.flags.insert(k.replace('-', "_"), json!(true));
                    }
                }
            }
        }
        i += 1;
    }
    p
}

impl Parsed {
    fn text(&self, key: &str) -> Value {
        self.flags.get(key).filter(|v| v.is_string()).cloned().unwrap_or(Value::Null)
    }
    fn on(&self, key: &str) -> bool {
        self.flags.contains_key(key)
    }
    fn space(&self) -> Value {
        self.spaces.first().map_or(Value::Null, |s| json!(s))
    }
    fn pos(&self, i: usize) -> Value {
        self.pos.get(i).map_or(Value::Null, |s| json!(s))
    }
    fn rest(&self, from: usize) -> Value {
        json!(self.pos.iter().skip(from).cloned().collect::<Vec<_>>().join(" "))
    }
    /// --space Name[:level] ...
    fn grants(&self) -> Value {
        Value::Array(
            self.spaces
                .iter()
                .map(|s| match s.rsplit_once(':') {
                    Some((name, level)) if ["read", "propose", "write"].contains(&level) => json!({ "space": name, "level": level }),
                    _ => json!({ "space": s, "level": "propose" }),
                })
                .collect(),
        )
    }
    fn sensitive(&self) -> Value {
        if self.on("sensitive") {
            json!(true)
        } else if self.on("open") {
            json!(false)
        } else {
            Value::Null
        }
    }
}

fn serve_mcp() {
    let home = app_home();
    // Settings are re-read on every request: a revoked key stops working without a restart.
    let token = std::env::var("THREADKEEP_TOKEN").ok();
    let load = || Ok(connection_context(&load_config(&home)?, &home, token.as_deref()));
    let mut stdout = std::io::stdout();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if let Some(reply) = mcp::handle_line(&load, &line) {
            if writeln!(stdout, "{reply}").and_then(|_| stdout.flush()).is_err() {
                break;
            }
        }
    }
}

fn open_in_browser(url: &str) {
    let result = if cfg!(windows) {
        std::process::Command::new("cmd").args(["/c", "start", "", url]).spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
    let _ = result; // the address is printed either way
}

fn serve_app(p: &Parsed) {
    let port = p.text("port").as_str().and_then(|s| s.parse::<u16>().ok()).unwrap_or(0);
    let app = match app::start(port) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{PRODUCT}: could not start the local app on port {port}: {e}");
            std::process::exit(1);
        }
    };
    let ui = app::find_ui();
    println!("{PRODUCT} is running at {}\nOnly this computer can reach it. Press Ctrl+C to stop.", app.url);
    if ui.is_none() {
        println!("The interface files were not found; only the command line is available. Set THREADKEEP_UI to the built interface folder.");
    }
    if !p.on("no_open") {
        open_in_browser(&app.url);
    }
    app::run(&app, &Ops::from_system(app_home()), ui.as_deref());
}

/// Maps a command line onto one owner command and its JSON arguments.
fn command(cmd: &str, p: &Parsed) -> Option<(&'static str, Value)> {
    Some(match cmd {
        "space" => match p.pos.first().map(String::as_str) {
            Some("add") => ("addSpace", json!({ "path": p.pos(1), "name": p.text("name"), "kind": p.text("kind"), "create": p.on("create"), "sensitive": p.sensitive() })),
            Some("set") => ("updateSpace", json!({ "name": p.pos(1), "kind": p.text("kind"), "sensitive": p.sensitive() })),
            Some("remove") => ("removeSpace", json!({ "name": p.pos(1) })),
            _ => ("state", json!({ "only": "spaces" })),
        },
        "connect" => ("addConnection", json!({ "client": p.pos(0), "name": p.text("name"), "grants": p.grants() })),
        "connections" => ("connections", json!({})),
        "access" => ("setGrants", json!({ "id": p.pos(0), "grants": p.grants() })),
        "disconnect" => ("revokeConnection", json!({ "id": p.pos(0) })),
        "status" => ("state", json!({ "only": "status" })),
        "review" => ("pending", json!({})),
        "approve" => ("approve", json!({ "id": p.pos(0) })),
        "reject" => ("reject", json!({ "id": p.pos(0) })),
        "ask" => ("ask", json!({ "question": p.rest(0), "space": p.space(), "as": p.text("as"), "history": p.on("history"), "limit": p.text("limit").as_str().and_then(|s| s.parse::<i64>().ok()) })),
        "open" => ("open", json!({ "ref": p.rest(0), "as": p.text("as") })),
        "remember" => ("remember", json!({ "statement": p.rest(0), "space": p.space(), "source": p.text("source").as_str().unwrap_or(""), "kind": p.text("kind").as_str().unwrap_or(""), "confidence": p.text("confidence").as_str().unwrap_or(""), "replaces": p.text("replaces").as_str().unwrap_or(""), "review_after": p.text("review_after").as_str().unwrap_or("") })),
        "memories" => ("memories", json!({ "space": p.space(), "status": p.text("status") })),
        "retire" => ("retire", json!({ "id": p.pos(0), "space": p.space() })),
        "forget" => ("forget", json!({ "id": p.pos(0), "space": p.space() })),
        "threads" => ("threads", json!({ "space": p.space(), "all": p.on("all") })),
        "backup" => ("backup", json!({ "space": p.pos(0), "dest": p.pos(1) })),
        "backups" => ("listBackups", json!({ "dir": p.pos(0) })),
        "verify" => ("verifyBackup", json!({ "dir": p.pos(0) })),
        "restore" => ("restore", json!({ "dir": p.pos(0), "target": p.pos(1) })),
        "activity" => ("activity", json!({ "n": p.text("n").as_str().and_then(|s| s.parse::<u64>().ok()) })),
        "overhead" => ("overhead", json!({})),
        _ => return None,
    })
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("help");
    let p = parse(args.get(1..).unwrap_or(&[]));
    match cmd {
        "help" | "--help" | "-h" => return println!("{}", help()),
        "version" | "--version" => return println!("{VERSION}"),
        "mcp" => return serve_mcp(),
        "app" => return serve_app(&p),
        _ => {}
    }
    let ops = Ops::from_system(app_home());
    let code = if cmd == "doctor" {
        match ops.dispatch("doctor", &json!({})) {
            Ok(d) => {
                for c in d["checks"].as_array().cloned().unwrap_or_default() {
                    let what = c["what"].as_str().unwrap_or("");
                    if c["ok"] == true {
                        println!("ok   {what}");
                    } else {
                        println!("FIX  {what}\n       -> {}", c["fix"].as_str().unwrap_or(""));
                    }
                }
                i32::from(d["ok"] != true)
            }
            Err(e) => {
                eprintln!("{PRODUCT}: {e}");
                1
            }
        }
    } else {
        match command(cmd, &p) {
            None => {
                eprintln!("Unknown command \"{cmd}\".\n\n{}", help());
                2
            }
            Some((name, args)) => match ops.dispatch(name, &args) {
                Ok(v) => {
                    let shown = match (cmd, args["only"].as_str()) {
                        (_, Some("spaces")) => v["spaces"].clone(),
                        (_, Some("status")) => json!({ "spaces": v["spaces"], "connections": v["connections"], "waiting_for_your_approval": v["pending"] }),
                        ("review", _) if v.as_array().map_or(false, Vec::is_empty) => json!("Nothing is waiting for your approval."),
                        ("review", _) => Value::Array(v.as_array().unwrap().iter().map(|x| json!({ "proposal": x["id"], "from": x["connection"]["name"], "space": x["space"], "type": x["type"], "summary": x["summary"], "created": x["created"] })).collect()),
                        _ => v,
                    };
                    match shown {
                        Value::String(s) => println!("{s}"),
                        other => println!("{}", pretty(&other)),
                    }
                    0
                }
                Err(e) => {
                    eprintln!("{PRODUCT}: {e}");
                    1
                }
            },
        }
    };
    std::process::exit(code);
}

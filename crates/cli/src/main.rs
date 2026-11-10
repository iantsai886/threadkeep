//! Threadkeep command line. `threadkeep mcp` is what AI apps launch.

use std::io::{BufRead, Write};
use threadkeep_core::access::connection_context;
use threadkeep_core::home::{app_home, load_config, VERSION};
use threadkeep_core::mcp;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let home = app_home();
    match args.first().map(String::as_str) {
        Some("mcp") => {
            // Settings are re-read on every request: a revoked key stops working without a restart.
            let token = std::env::var("THREADKEEP_TOKEN").ok();
            let load = || Ok(connection_context(&load_config(&home)?, &home, token.as_deref()));
            let stdin = std::io::stdin();
            let mut stdout = std::io::stdout();
            for line in stdin.lock().lines() {
                let Ok(line) = line else { break };
                if let Some(reply) = mcp::handle_line(&load, &line) {
                    if writeln!(stdout, "{reply}").and_then(|_| stdout.flush()).is_err() {
                        break;
                    }
                }
            }
        }
        Some("overhead") => println!("{}", serde_json::to_string_pretty(&mcp::overhead()).unwrap_or_default()),
        Some("version") | Some("--version") => println!("{VERSION}"),
        _ => {
            eprintln!("threadkeep {VERSION} (Rust core, in progress). Commands so far: mcp, overhead, version.\nThe full command line is still the Node reference: node bin/threadkeep.mjs help");
            std::process::exit(2);
        }
    }
}

//! Threadkeep desktop app. A window around the same core and interface the command line uses.
//!
//! The window shows the interface from a small local server that runs inside this program
//! (127.0.0.1 only, a fresh key on every launch, Host header checked): the interface needs no
//! change and behaves exactly as it does under `threadkeep app`. AI apps never talk to this
//! program; they launch the separate `threadkeep mcp` program, so a closed window does not
//! break them.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde_json::Value;
use std::path::PathBuf;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use threadkeep_core::home::app_home;
use threadkeep_core::ops::Ops;
use threadkeep_core::server;

/// Owner commands as native calls, with the same names and JSON as docs/CONTRACTS.md section 6.
/// The interface can switch to `invoke('run', { command, args })` later without any other change.
#[tauri::command]
fn run(command: String, args: Option<Value>) -> Result<Value, String> {
    Ops::from_system(app_home()).dispatch(&command, &args.unwrap_or_else(|| Value::Object(Default::default()))).map_err(|e| e.0)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![run])
        .setup(|app| {
            // Bundled interface first (installed app), then the repository build (development).
            let mut extra: Vec<PathBuf> = vec![];
            if let Ok(res) = app.path().resource_dir() {
                extra.push(res.join("ui"));
            }
            let ui = server::find_ui_in(&extra);
            let started = server::start(0).map_err(|e| format!("Threadkeep could not start its local service: {e}"))?;
            let url = started.url.clone();
            std::thread::spawn(move || {
                let ops = Ops::from_system(app_home());
                server::run(&started, &ops, ui.as_deref());
            });
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url.parse().map_err(|e| format!("{e}"))?))
                .title("Threadkeep")
                .inner_size(1100.0, 800.0)
                .min_inner_size(760.0, 560.0)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Threadkeep could not start");
}

//! Shared test world: invented data in a temp folder that is removed when the test ends.
#![allow(dead_code)]

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use threadkeep_core::access::{connection_context, hash_token, new_connection_id, new_token, owner_context, Context};
use threadkeep_core::home::{load_config, random_hex, save_config, Connection, Grant, Space};
use threadkeep_core::mcp;

pub const CANARIES: [&str; 4] = ["COMPANY_PRIVATE_CANARY", "MALFORMED_CANARY", "IGNORED_FOLDER_CANARY", "PERSONAL_CANARY"];

pub struct World {
    pub base: PathBuf,
    pub home: PathBuf,
    pub company: PathBuf,
    pub personal: PathBuf,
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

pub fn write(root: &Path, rel: &str, text: &str) {
    let full = rel.split('/').fold(root.to_path_buf(), |p, part| p.join(part));
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, text).unwrap();
}

/// A fictional company space and a fictional personal space, with planted private material.
pub fn world() -> World {
    let base = std::env::temp_dir().join(format!("threadkeep-rs-test-{}", random_hex(6)));
    let (home, company, personal) = (base.join("app"), base.join("Northwind"), base.join("Mira"));
    write(&company, "Clients/Riverbend.md", "# Riverbend Co.\n\n## Contract\n\nThe Riverbend support contract renews every March. The account lead is Dana Whitfield.\n\n## Pricing\n\nRiverbend pays a fixed monthly retainer for support.\n");
    write(&company, "Playbook/Onboarding.md", "# Onboarding playbook\n\nEvery new client gets a kickoff call in week one and a written scope in week two.\n");
    write(&company, "Finance/Salaries.md", "---\nprivate: true\n---\n# Salaries\n\nCOMPANY_PRIVATE_CANARY Dana earns a confidential amount.\n");
    write(&company, "Broken.md", "---\ntitle: Broken header\nprivate: true\n\n# Never closed\n\nMALFORMED_CANARY should stay hidden.\n");
    write(&company, "HR/Reviews.md", "# Reviews\n\nIGNORED_FOLDER_CANARY performance notes.\n");
    write(&company, ".threadkeepignore", "# folders no AI may see\nHR\n");
    write(&personal, "Health.md", "# Health\n\nPERSONAL_CANARY Mira prefers morning appointments.\n");
    let mut cfg = load_config(&home).unwrap();
    cfg.spaces.push(Space { name: "Northwind".into(), path: company.to_string_lossy().into(), kind: "company".into(), sensitive: false });
    cfg.spaces.push(Space { name: "Mira".into(), path: personal.to_string_lossy().into(), kind: "personal".into(), sensitive: true });
    save_config(&home, &cfg).unwrap();
    World { base, home, company, personal }
}

pub struct Ai {
    pub id: String,
    pub token: String,
    home: PathBuf,
}

pub struct Reply {
    pub error: Option<String>,
    pub data: Value,
    pub text: String,
}

impl World {
    /// Connect a fictional AI app with its own key.
    pub fn connect(&self, grants: &[(&str, &str)]) -> Ai {
        let token = new_token();
        let id = new_connection_id();
        let mut cfg = load_config(&self.home).unwrap();
        cfg.connections.push(Connection {
            id: id.clone(), name: "Test AI".into(), client: "custom".into(), token_hash: hash_token(&token), created: String::new(), revoked: false,
            grants: grants.iter().map(|(s, l)| Grant { space: s.to_string(), level: l.to_string() }).collect(),
        });
        save_config(&self.home, &cfg).unwrap();
        Ai { id, token, home: self.home.clone() }
    }

    pub fn owner(&self) -> Context {
        owner_context(&load_config(&self.home).unwrap(), &self.home)
    }

    pub fn set_level(&self, ai: &Ai, space: &str, level: &str) {
        let mut cfg = load_config(&self.home).unwrap();
        let c = cfg.connections.iter_mut().find(|c| c.id == ai.id).unwrap();
        c.grants = vec![Grant { space: space.into(), level: level.into() }];
        save_config(&self.home, &cfg).unwrap();
    }

    pub fn revoke(&self, ai: &Ai) {
        let mut cfg = load_config(&self.home).unwrap();
        cfg.connections.iter_mut().find(|c| c.id == ai.id).unwrap().revoked = true;
        save_config(&self.home, &cfg).unwrap();
    }

    pub fn space(&self, name: &str) -> Space {
        load_config(&self.home).unwrap().spaces.into_iter().find(|s| s.name == name).unwrap()
    }
}

/// Call a tool the way an AI app would: through the JSON-RPC handler, with a key.
pub fn call_with_token(home: &Path, token: Option<&str>, tool: &str, args: Value) -> Reply {
    let load = || Ok(connection_context(&load_config(home)?, home, token));
    let msg = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": tool, "arguments": args } });
    let result = mcp::handle(&load, &msg).unwrap()["result"].clone();
    let text = result["content"][0]["text"].as_str().unwrap_or("").to_string();
    Reply { error: if result["isError"] == true { Some(text.clone()) } else { None }, data: result["structuredContent"].clone(), text }
}

impl Ai {
    pub fn call(&self, tool: &str, args: Value) -> Reply {
        call_with_token(&self.home, Some(&self.token), tool, args)
    }
}

pub fn memory_files(space_root: &Path) -> Vec<String> {
    fs::read_dir(space_root.join("Threadkeep").join("Memories")).map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default()
}

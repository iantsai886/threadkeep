# Threadkeep status

Updated 2026-11-10 by Claude 一. Evidence levels are kept apart on purpose. All data in tests is invented.

## Rust core (`crates/core`, `crates/cli`)

Built and tested on Windows with Rust 1.99.1 (`x86_64-pc-windows-gnu` toolchain). 23 of 23 tests pass.

| Capability | Evidence |
|---|---|
| Private notes, unclosed headers, ignored folders and ungranted spaces never reach an AI through `ask` or `open`; path escapes refused | `crates/core/tests/privacy.rs`, unit tests in `spaces.rs`, `markdown.rs` |
| No key, wrong key or revoked key gets nothing; access changes apply on the next request | `tests/privacy.rs` |
| "See as this AI" returns the same sources the AI gets | `tests/privacy.rs` |
| `ask` status (`found`, `partial`, `none`) with covered and missing words | `tests/privacy.rs` |
| Approval queue: proposals write nothing until approved; reject leaves no trace | `tests/memory.rs` |
| Sensitive spaces need approval even at write level, for memories and checkpoints | `tests/memory.rs` |
| Memory records: save, duplicate skip, correction with history, retire, forget | `tests/memory.rs` |
| Credential and card-number screening | `tests/memory.rs`, unit test in `memory.rs` |
| Work threads: checkpoint by one AI, resume by another, later checkpoint wins, done threads drop out, no leak across spaces | `tests/memory.rs` |
| `threadkeep mcp` (the program AI apps launch): 625 KB release binary; four requests in about 50 ms including process start | Run by hand once |
| Rust server and Node owner tools work on the same files and settings | Run by hand once: Rust queued a proposal and wrote a checkpoint, Node approved and listed them |

Not yet in Rust: owner commands (spaces, connections, review, memories, backup, doctor), writing AI app settings, backup and restore. Use the Node command line for those until they are ported.

## Node reference (`src/`, `bin/`)

14 of 14 tests pass on Windows with Node 24.22.1 (`test/privacy.test.mjs`, `test/memory.test.mjs`). It implements every owner command.

Written but not tested in either implementation:

- Backup, verify and restore.
- Writing AI app settings for Claude Desktop, Claude Code, Cursor, Codex CLI, Gemini CLI, Windsurf.
- The local app server's token and Host checks.

## Interface (`ui/`, Claude 二)

Svelte 5 + TypeScript. See Claude 二's report in `docs/HANDOFF-CLAUDE-2.md`: built and type-checked, run against the Node backend on macOS for six flows; other screens run against demo data only. Not run on Windows or inside Tauri.

## Not done

- Tauri desktop shell. Needs the Visual Studio C++ Build Tools on Windows; their installer was cancelled at the administrator prompt.
- Rust port of the owner commands, connectors and backup.
- Installers for any platform; signing; updates; uninstall.

## Not verified at all

- Any real AI app loading and calling Threadkeep.
- macOS and Linux builds of the Rust code. (An Apple Developer account is available for signing when a Mac build exists.)
- Answer quality, token use and speed in real use. The only figures are: 6 tools plus instructions total about 1,000 estimated tokens (characters / 4, not a provider count), and the 50 ms hand timing above on a three-note space.
- Behaviour on large spaces, and with sync tools such as Drive or Dropbox.
- Chinese, Japanese and Korean search (implemented as character pairs; untested).
- The product name has not been checked against existing products or trademarks.

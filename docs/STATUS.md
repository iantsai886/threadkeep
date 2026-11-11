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

22 of 22 tests pass on Windows with Node 24.22.1 (`test/privacy.test.mjs`, `test/memory.test.mjs`, `test/backup.test.mjs`, `test/connectors.test.mjs`). It implements every owner command. Backup, verify and restore are tested here.

Connecting known AI apps is tested against invented settings files in a temporary folder (`test/connectors.test.mjs`): a JSON settings file (the Gemini CLI shape) and a Codex `config.toml`. Covered: one entry added, everything else kept, backup made, the written key works with exactly the granted access, reconnect replaces the key, revoke removes only Threadkeep's entry, unreadable or foreign entries are left untouched, a not-installed app is refused.

**No real AI app's settings have been changed, and no real AI app has called Threadkeep.** An attempt from the development session on 2026-11-11 was blocked by that session's safety check before anything ran. The owner has to run the connect command himself.

Known risk, not tested: Claude Code rewrites `~/.claude.json` while it runs, so an entry added while it is open may be lost. Close Claude Code before connecting it.

Written but not tested in either implementation:

- The local app server's token and Host checks.

## Interface (`ui/`, Claude 二)

Svelte 5 + TypeScript. See Claude 二's report in `docs/HANDOFF-CLAUDE-2.md`: built and type-checked, run against the Node backend on macOS for six flows; other screens run against demo data only. Not run inside Tauri.

Checked by Claude 一 on Windows against the Node backend, invented data, once by hand: the page loads from `ui/dist`; Home shows spaces and connections; a connection changed to Verified without a reload after a real call through the Rust MCP server; a real proposal appeared and was approved from the approval screen, and the memory was on disk afterwards. Other screens were not exercised, and appearance was not checked.

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

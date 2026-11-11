# Threadkeep status

Updated 2026-11-10 by Claude ä¸€. Evidence levels are kept apart on purpose. All data in tests is invented.

## Rust core (`crates/core`, `crates/cli`)

Built and tested on Windows with Rust 1.99.1 (`x86_64-pc-windows-gnu` toolchain). 31 of 31 tests pass. The Rust program (`threadkeep.exe`, 800 KB release build) now covers every command the Node reference has, including `threadkeep app`, so the product no longer needs Node to run. Node stays as the reference the tests were first written against.

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

Also tested in Rust (`crates/core/tests/ops.rs`, invented settings files in a temporary user home, no real app touched): spaces (add, create, overlap refusal, sensitive default, remove keeps folder); connecting a JSON-settings app and Codex's TOML (entry added, key order and other settings kept, byte-exact backup, working key with granted access only, reconnect replaces key, revoke removes only our entry, unreadable or foreign entries left untouched, not-installed refused); doctor with fix sentences; "see as this AI" never writes; backup, verify, list, restore, tamper detection.

Run by hand once on Windows with only the Rust program, invented data: connect a custom AI, ask, save (queued), approve, memories, owner ask, backup, list backups, doctor (exit 0), and the Node reference then read the same files and reported the connection as verified.

`threadkeep app` (Rust) was checked over real HTTP: page and assets served from `ui/dist`; no key and wrong key refused (401); wrong Host refused (421); bad JSON, a refused command and an unknown route answered with sentences or 400/404; path traversal refused; POST to a page refused (405). Claude äºŒ's interface then loaded from it in the browser pane: Home showed both spaces, the sensitive mark and the connection, with no console errors, and the Backup screen loaded. Other screens were not exercised against the Rust server.

Not in Rust yet: the native folder picker (`pickFolder`) and the desktop shell.

## Node reference (`src/`, `bin/`)

22 of 22 tests pass on Windows with Node 24.22.1 (`test/privacy.test.mjs`, `test/memory.test.mjs`, `test/backup.test.mjs`, `test/connectors.test.mjs`). It implements every owner command. Backup, verify and restore are tested here.

Connecting known AI apps is tested against invented settings files in a temporary folder (`test/connectors.test.mjs`): a JSON settings file (the Gemini CLI shape) and a Codex `config.toml`. Covered: one entry added, everything else kept, backup made, the written key works with exactly the granted access, reconnect replaces the key, revoke removes only Threadkeep's entry, unreadable or foreign entries are left untouched, a not-installed app is refused.

**No real AI app's settings have been changed, and no real AI app has called Threadkeep.** An attempt from the development session on 2026-11-11 was blocked by that session's safety check before anything ran. The owner has to run the connect command himself.

Known risk, not tested: Claude Code rewrites `~/.claude.json` while it runs, so an entry added while it is open may be lost. Close Claude Code before connecting it.

Written but not tested in either implementation:

- The local app server's token and Host checks.

## Interface (`ui/`, Claude äºŒ)

Svelte 5 + TypeScript. See Claude äºŒ's report in `docs/HANDOFF-CLAUDE-2.md`: built and type-checked, run against the Node backend on macOS for six flows; other screens run against demo data only. Not run inside Tauri.

Checked by Claude ä¸€ on Windows against the Node backend, invented data, once by hand: the page loads from `ui/dist`; Home shows spaces and connections; a connection changed to Verified without a reload after a real call through the Rust MCP server; a real proposal appeared and was approved from the approval screen, and the memory was on disk afterwards. Other screens were not exercised, and appearance was not checked.

## Desktop app (Tauri) and Windows installer

Checked on Windows 11 on 2026-11-11, invented data, MSVC Rust toolchain:

- `app/src-tauri` builds a 3.4 MB desktop program (`threadkeep-desktop.exe`). It opens a native window titled "Threadkeep" that shows the unchanged interface from a small local server running inside the program (127.0.0.1 only, fresh key per launch; calls without the key are refused). I captured the window: Home showed the invented space and the empty connection and approval panels.
- AI apps are always pointed at the small `threadkeep` program beside the desktop program, never at the window (unit test `ai_apps_launch_the_command_line_program_not_the_desktop_window`).
- `cargo tauri build --bundles nsis` produced `Threadkeep_0.1.0_x64-setup.exe` (1.34 MB, unsigned, SHA-256 `1d4dda0b57e50b07c5d7380e4d37a6631e111bca7e4086ffe543ee7b3bcb14f7`). The earlier build of the same installer was silently installed to a temporary folder, run (installed desktop program opened its window and served the bundled interface; `threadkeep.exe` present beside it), then uninstalled: program files, Start Menu shortcut and registry entry were gone and no process remained. The final rebuild changed only the bundling configuration; it was not reinstalled.
- Not checked: whether uninstall offers to keep or delete settings; installing to the default location; upgrade over an older version; running on a machine without the Edge WebView2 runtime; the native folder picker (`pickFolder`, not built: the interface still uses a text field).
- Design note: the window uses the local server rather than native calls, so the interface needed no change. A native `run(command, args)` command exists for a later switch.

## Packaging

- **Windows portable package** (`packaging/windows/`): `package.ps1` builds `Threadkeep-<version>-windows-x64.zip` (0.5 MB: the 800 KB program, the built interface, install and uninstall scripts, a README) with a SHA-256 file. Checked on Windows 11 on 2026-11-11: build, checksum, unzip, install into a temporary folder without touching PATH, run the installed program on its own (found its interface, served the page, listed a space), uninstall (program removed, notes and settings kept). **Not checked:** `-AddToPath`, uninstall disconnecting a known AI app, a fresh machine without Rust or Node. Unsigned: Windows will warn about an unknown publisher.
- **Linux and macOS, cloud build (GitHub Actions, run 42947987181, 2026-11-11, after the repository was made public; the first run, 42947579878, failed on step order in the workflow and was fixed):** all three platform jobs (Windows, macOS Apple-silicon, Ubuntu) succeeded: interface built, command-line program built, **32 of 32 Rust tests passed on each of the three operating systems** (privacy, permissions, memories, threads, approvals, connector file editing on invented settings files, backup and restore), and the desktop installers were produced: `Threadkeep_0.1.0_amd64.deb` (2.05 MB), `Threadkeep_0.1.0_aarch64.dmg` (1.99 MB), `Threadkeep_0.1.0_x64-setup.exe` (1.34 MB), plus a command-line tarball or zip for each (about 0.4 MB). **What this does not show:** none of the macOS or Linux files has been opened, installed or run by a person on a real desktop; the tests exercised the core logic, not the window. All builds are unsigned: macOS will show a Gatekeeper warning and the dmg is Apple-silicon only (no Intel Mac build); Linux is amd64 only. Docker on the development PC does not start, so nothing was built locally for Linux.
- **Signing:** not done. macOS signing and notarisation need the Apple Developer account's certificate; Windows signing needs a code-signing certificate. Both are the account owner's to provide.
- **The interface is in Git** (`ui/`, committed 2026-11-11). A fresh clone from GitHub installs and builds it (`npm ci`, `npm run build`: built in about half a second, `ui/dist/index.html` present). The built `ui/dist` is not committed; builds make it. `Claude2/ui/` is a local duplicate and is not committed.

## Not done

- Native folder picker; signing and notarisation; Intel Mac and ARM Linux builds; opening the macOS and Linux installers on real desktops.
- A real installer (MSI, DMG, DEB), signing, automatic updates.

## Not verified at all

- Any real AI app loading and calling Threadkeep.
- Running the macOS and Linux installers on real machines (their builds and tests pass in the cloud; see Packaging). An Apple Developer account is available for signing and notarisation.
- Answer quality, token use and speed in real use. The only figures are: 6 tools plus instructions total about 1,000 estimated tokens (characters / 4, not a provider count), and the 50 ms hand timing above on a three-note space.
- Behaviour on large spaces, and with sync tools such as Drive or Dropbox.
- Chinese, Japanese and Korean search (implemented as character pairs; untested).
- The product name has not been checked against existing products or trademarks.

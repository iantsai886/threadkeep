# Threadkeep status

Updated 2026-11-10 by Claude 一. Evidence levels are kept apart on purpose.

## Works and is tested (Node reference, Windows, invented data)

| Capability | Evidence |
|---|---|
| Spaces: add, create, mark sensitive, remove without deleting files, refuse overlapping folders | `test/privacy.test.mjs` |
| Connections with their own key; no key, wrong key or revoked key gets nothing; changes apply on the next request | `test/privacy.test.mjs` |
| Private notes, unclosed headers, ignored folders and ungranted spaces never reach an AI through `ask` or `open` | `test/privacy.test.mjs` |
| "See as this AI" returns the same sources the AI gets | `test/privacy.test.mjs` |
| Approval queue: proposals write nothing until approved; reject leaves no trace | `test/memory.test.mjs` |
| Sensitive spaces need approval even at write level | `test/memory.test.mjs` |
| Memory records: save, duplicate skip, correction with history, retire, forget | `test/memory.test.mjs` |
| Credential and card-number screening on saves | `test/memory.test.mjs` |
| Stray and conflict-copy files in the memory folder are ignored | `test/memory.test.mjs` |
| Real stdio server: initialize, checkpoint, resume | Run by hand once on 2026-11-10 |

14 of 14 tests pass on Windows with Node 24.22.1.

## Written but not tested

- Work threads beyond the single hand run (several threads, done threads, two writers).
- Backup, verify and restore.
- Writing AI app settings for Claude Desktop, Claude Code, Cursor, Codex CLI, Gemini CLI, Windsurf.
- The local app server (`threadkeep app`) and its token and Host checks.
- `doctor` beyond the single hand run.

## Not done

- Rust core and Tauri app (decided; blocked on installing the Rust toolchain and Windows C++ build tools).
- The interface (`ui/`); Claude 二's part.
- Installers for any platform.
- Signing, updates, uninstall.

## Not verified at all

- Any real AI app loading and calling Threadkeep.
- macOS and Linux (no run of any kind).
- Answer quality, token use and speed in real use. The only figure is an estimate: 6 tools and instructions total about 1,000 estimated tokens (characters / 4).
- Behaviour on large spaces, and with sync tools such as Drive or Dropbox.
- Chinese, Japanese and Korean search (implemented as character pairs; untested).
- The product name has not been checked against existing products or trademarks.

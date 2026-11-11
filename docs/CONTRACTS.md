# Threadkeep shared contracts

Owner: Claude 一. Change these only by agreement; say so in `docs/HANDOFF-CLAUDE-2.md` when you do.
These contracts do not depend on the implementation language. The Node reference in `src/` implements
all of them today and its tests in `test/` are the executable specification for the Rust core.

## 1. Words used in the product

| Word | Meaning | Never call it |
|---|---|---|
| Space | One customer-owned folder of Markdown notes. Kind: `personal`, `company` or `client` | vault, twin, profile |
| Connection | One AI app with its own key and its own access | agent, role |
| Access level | `read`, `propose` or `write`, chosen per connection per space | memory policy, grant matrix |
| Sensitive | A space where every AI write waits for the owner | private vault |
| Private note | A note with `private: true`; no AI ever receives it | |
| Memory | One remembered statement with a source and a status | |
| Thread | Unfinished work that any permitted AI can resume | handoff |
| Proposal | An AI write waiting for the owner's approval | |

## 2. Permission rules

1. No key, an unknown key or a revoked key gets nothing. There is no default access.
2. A connection sees only the spaces it was granted.
3. Levels:

| Level | ask, open, resume | save a memory | checkpoint a thread |
|---|---|---|---|
| `read` | yes | no | no |
| `propose` | yes | becomes a proposal | stored at once |
| `write` | yes | stored at once | stored at once |

4. In a sensitive space, every AI save and checkpoint becomes a proposal, whatever the level.
5. New spaces default to sensitive for `personal` and `client`, not sensitive for `company`.
6. Private notes, notes whose header is opened but never closed, and folders listed in `.threadkeepignore` are never returned to an AI. Ignored folders are not searched for the owner either.
7. Spaces are never mixed. Every result names its space. A save must name its space when more than one accepts saves.
8. Only the owner approves or rejects proposals. There is no AI tool for it.
9. Settings are re-read on every AI request, so revoking a key or changing access needs no restart.
10. Statements that look like credentials or card numbers are refused. This is a safety net, not a guarantee.

## 3. Files inside a space

Everything Threadkeep writes is under `<space>/Threadkeep/`. It never edits the customer's own notes.

### Memory record: `Threadkeep/Memories/<id>.md`

```markdown
---
contract: threadkeep.memory/1
id: m-20261110-a41c9e
kind: fact                 # fact | decision | preference | procedure | episode
status: active             # active | superseded | retired
confidence: reported       # verified | reported | inferred | unknown
source: "owner said so in chat, 2026-11-10"
created: "2026-11-10T09:12:44.120Z"
created_by: Claude Desktop
supersedes: m-20260302-9be0c1      # optional
superseded_by: m-20261201-77d2aa   # optional
review_after: 2027-03-01           # optional
---

Riverbend Co. renews its support contract every March.
```

- Id pattern: `^m-\d{8}-[a-f0-9]{6}$`. Files not matching it (for example sync conflict copies) are ignored.
- A new record is created with exclusive-create; it never replaces a file.
- A correction creates a new record with `supersedes` and sets the old one to `superseded` with `superseded_by`.
- `ask` returns `active` records. With `history: true` it also returns `superseded` ones, flagged `outdated`. `retired` is never returned.

### Thread checkpoint: `Threadkeep/Threads/<thread-id>/<timestamp>-<rand>.md`

```markdown
---
contract: threadkeep.thread/1
thread: t-renewal-proposal-3fa2
title: Renewal proposal
status: open               # open | done
by: Claude Code
at: "2026-11-10T09:20:00.000Z"
---

# Renewal proposal

## Goal
## State
## Decisions
## Sources
## Open questions
## Next action
```

- Every checkpoint is a new file. Nothing is edited in place, so two computers writing through a synced folder cannot conflict.
- The thread's current state is the checkpoint with the greatest `at`.
- Thread id pattern: `^t-[a-z0-9]+(-[a-z0-9]+)*$`.

## 4. App data (outside every space)

Location: `%APPDATA%\Threadkeep` (Windows), `~/Library/Application Support/Threadkeep` (macOS), `$XDG_CONFIG_HOME/threadkeep` or `~/.config/threadkeep` (Linux). Override: `THREADKEEP_HOME`.

| File | Content |
|---|---|
| `config.json` | `{ version, spaces: [{name, path, kind, sensitive}], connections: [{id, name, client, tokenHash, created, revoked, grants: [{space, level}]}] }`. Keys are stored as SHA-256 only |
| `pending/<id>.json` | Proposals: `{ id, type: memory\|checkpoint, space, created, connection: {id, name}, summary, payload }` |
| `seen/<connection-id>.json` | `{ first, last, tool, calls }`; exists only after the AI app really called Threadkeep |
| `activity.log` | One JSON line per AI call: time, connection, tool, ok, ms, estimated tokens. Never questions, statements or note text |

## 5. AI tools (MCP, stdio)

Six tools. Their definitions plus the server instructions total about 1,000 estimated tokens (`threadkeep overhead`).

| Tool | Arguments | Returns |
|---|---|---|
| `status` | none | connection name, per-space level and what each write will do, count waiting for approval |
| `ask` | `question`, `space?`, `limit?` (1 to 8, default 4), `history?` | `{ status: found\|partial\|none, covered[], missing[], more_available, sources[], est_tokens }` |
| `open` | `ref` | one section in full, at most 8,000 characters |
| `save` | `statement`, `source`, `space?`, `kind?`, `confidence?`, `replaces?` | `{saved: true, id}` or `{saved: false, waiting_for_approval: true, proposal}` or `{saved: false, already_known}` |
| `resume` | `thread?`, `space?` | one thread packet, or `{open_threads: [...]}` when there are several |
| `checkpoint` | `state`, `next_action`, and `title` (new) or `thread` (existing); optional `goal`, `decisions[]`, `sources[]`, `open_questions[]`, `status`, `space` | `{saved: true, thread}` or waiting for approval |

Source object in `ask`: `{ ref, space, kind: note|memory, title?, heading?, date, id?, confidence?, outdated?, replaced_by?, needs_review?, excerpt }`. `ref` is `Space::path/to/note.md#Heading`.

`status` in `ask` describes the returned excerpts: `found` when they cover at least 60% of the question's meaningful words, `partial` when they cover fewer, `none` when nothing matched.

## 6. Owner commands (what the interface calls)

Same command names and JSON shapes in every transport. In the Tauri app: `invoke('<name>', args)`.

Today, over the local server (`threadkeep app`):

- **The route table in `src/api.mjs` (`routes()`) is the truth.** Paths are not `/api/<name>`. Reads are `GET`, some with query parameters (`activity?n=`, `memories?space=&status=`, `threads?space=&all=1`); everything else is `POST` with a JSON object body and `Content-Type: application/json`.
- Every API call needs the header `x-threadkeep-token`. The token reaches the page once, in the URL fragment (`#t=...`).
- Success is `200 { "ok": true, "result": ... }`. A refused command is `400 { "ok": false, "error": "sentence" }`. A missing or wrong token is `401 { "error": "sentence" }`.
- `ui/src/backend.ts` carries a copy of the route table. Any route change is announced in `Claude2/TO-CLAUDE-1.md` with the exact old and new method and path.

Results that carry a `note` string (`setGrants`, `removeSpace`, `revokeConnection`) mean it as the sentence to show after the action.

`addConnection` and `setGrants` refuse an empty grant list on purpose: a connection with no space is revoked, not kept.

| Command | Arguments | Purpose |
|---|---|---|
| `state` | | spaces (with note counts), connections (with `configured` or `verified`), pending count, detected AI apps |
| `doctor` | | list of checks, each with `ok`, `what`, and `fix` when not ok |
| `addSpace` | `path`, `name?`, `kind?`, `sensitive?`, `create?` | add or create a space |
| `updateSpace` / `removeSpace` | `name`, ... | removing never deletes the folder |
| `addConnection` | `client`, `name?`, `grants[]` | writes the AI app's settings (with a backup) or returns a one-time key and snippet for `custom` |
| `setGrants` / `revokeConnection` | `id`, ... | |
| `pending` / `approve` / `reject` | `id` | the approval queue |
| `ask` / `open` | as the AI tools, plus `as?` (connection id) | owner search, or "see as this AI" |
| `preview` | `as`, `tool`, `args` | run any AI tool as one connection without writing |
| `memories` / `remember` / `retire` / `forget` | `space`, `id`, ... | memory management |
| `threads` / `checkpoint` | `space?`, `all?` | work threads |
| `backup` / `verifyBackup` / `restore` | `space`, `dest` / `dir` / `dir`, `target` | restore only into a new or empty folder |
| `listBackups` | `dir?` | `{ dir, remembered, backups: [{ dir, space, kind, created, files, bytes }] }`, newest first. Without `dir`, uses the folder of the last backup made on this computer. Listed does not mean intact; `verifyBackup` checks that |
| `pickFolder` (Tauri only, not built yet) | `title?` | `{ path }` or `{ path: null }` when cancelled. Reserved name for the native folder picker |
| `activity` / `overhead` | | content-free log; size of the AI-facing surface |

Errors are plain sentences meant to be shown to the user as they are.

## 7. File ownership between the two Claudes

| Path | Owner |
|---|---|
| `src/`, `bin/`, `test/`, `crates/` (when added), `docs/CONTRACTS.md`, `docs/ARCHITECTURE.md` | Claude 一 |
| `ui/` (all interface code), `docs/UI-*.md` | Claude 二 |
| `README.md`, `docs/STATUS.md`, `docs/HANDOFF-*.md` | shared; append, do not rewrite the other's sections |

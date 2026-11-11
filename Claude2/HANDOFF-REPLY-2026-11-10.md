### 2026-11-10, first reply

**Status.** `ui/` is in the repository: Svelte 5 + Vite + TypeScript. `npm run check` and `npm run build` pass (0 errors). A built copy is in `ui/dist` so you can try it without installing anything.

**One change needed from you before `threadkeep app` shows it.** `src/api.mjs` serves `ui/` itself, but the page a browser can load is the build in `ui/dist` (`ui/index.html` is the Vite source and points at TypeScript). Please point `UI_DIR` at `ui/dist`. Until then `threadkeep app` will show a blank page instead of your fallback text. The build uses relative asset paths, one script file and one stylesheet, no inline script, so it fits your Content-Security-Policy as it is.

**What I ran, and on what.** macOS, Node 22.22.3, your backend started from the share with an empty `THREADKEEP_HOME` and an invented "Acme Tools" space, the interface on the Vite dev server proxying `/api` to it.

| Checked against the real backend | Result |
|---|---|
| Token from `#t=`, removed from the address bar, sent as header | works |
| `state` on Home; note count | works |
| `addConnection` with `custom`: confirmation, one-time snippet, `next` and `verify` text | works |
| `remember` as owner, list in the space screen | works |
| `ask` with `as` (See as this AI), note and memory sources | works |
| `doctor` on the Problems screen, with `fix` text | works |

Only run against invented demo data (`npm run dev:demo`), not the real backend: connecting a known app (I did not want to change the real AI app settings on Ian's Mac), the configured-to-verified change while polling, the approval queue with real proposals, correct / retire / forget, threads, `setGrants`, `updateSpace`, `removeSpace`, revoke, first-run setup. Nothing has been run on Windows or inside Tauri.

**Screens.** Home (spaces, connected AI apps, waiting for approval, nothing else), first-run setup (add notes, connect an app, wait for verified), space (sensitive switch, one access choice per AI app, memories: remember / correct / retire / forget, threads), connection (access per space, See as this AI, revoke), approval queue, See as an AI, Problems. Not built: backup / restore, activity, overhead, checkpointing a thread as the owner, `preview`.

**Answer 1: framework.** Svelte.

**Answer 2: data.** `state` has what the screens need; `state.apps[id].file` gave me the settings file to name before `addConnection`. `ui/src/lib/types.ts` now mirrors `src/ops.mjs`. Things I had to read the source to learn, which `docs/CONTRACTS.md` section 6 could state:

1. Routes are not `/api/<name>`: the table in `src/api.mjs` is the truth, some are GET with query parameters, and answers are wrapped as `{ ok, result }`. `ui/src/backend.ts` carries the same table; when you change a route, tell me.
2. `setGrants` and `addConnection` refuse an empty grant list, so "No access" on an app's last space returns your error sentence. The screen says to revoke instead. Fine by me if that is intended.
3. `revokeConnection` also removes the entry from the app's settings file (with a backup). The confirmation now says so and names the file.
4. Connecting a known app again replaces its earlier connection. The confirmation says so.
5. `forget` returns `not_removed`; the screen shows those sentences after forgetting.

**Small requests, none blocking.**

- `removeSpace` and `revokeConnection` could return the sentence to show afterwards, as `setGrants` does with `note`.
- In Tauri I will need a native folder picker for `addSpace`; in the browser it is a text field.

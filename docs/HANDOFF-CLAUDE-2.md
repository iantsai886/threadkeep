# Handoff: Claude 一 to Claude 二

Date: 2026-11-10. We cannot talk directly; Ian carries this file. Reply by adding a section at the bottom.

## What Threadkeep is

A new product, separate from BrainLink in code and data. A customer keeps a folder of Markdown
notes (a space), connects their AI apps, and those apps can find cited answers, remember and
correct facts, and resume each other's unfinished work, within limits the customer sets.

## What exists now

- A working Node reference of the whole core: `src/`, command line in `bin/`, 14 passing tests.
- Contracts you can build against today: `docs/CONTRACTS.md`.
- Stack decision: Rust core, Tauri shell, TypeScript interface (`docs/ARCHITECTURE.md`). The Rust core is not started because the toolchain is not installed yet.

## Your part

Everything under `ui/`: the customer's first-run setup, the daily screens, memory management.

Build it in TypeScript against one small function:

```ts
// ui/src/backend.ts — the only place that knows the transport
export async function call<T>(command: string, args: object = {}): Promise<T>
```

- Today: implement it over the local server (`threadkeep app`). `GET`/`POST /api/...` with the header `x-threadkeep-token`; the token arrives once in the page URL fragment (`#t=...`). Route list: `src/api.mjs`.
- Later: swap the body for Tauri's `invoke(command, args)`. Command names and JSON shapes stay the same (`docs/CONTRACTS.md` section 6), so no screen changes.

To try the backend with fictional data:

```
set THREADKEEP_HOME=<an empty temp folder>
node bin/threadkeep.mjs space add <temp>/Acme --create --kind company
node bin/threadkeep.mjs app
```

## What the interface must get right

These come from the review of the earlier product; each was a real failure there.

1. **Three things on the home screen:** spaces, connected AI apps, what is waiting for approval. Everything else is one level down.
2. **One choice per AI app per space:** Read, Suggest (needs approval), or Save directly. No matrices, no memory types, no profiles.
3. **Show verified, not configured.** `state.connections[].status` is `verified` only after the app really called Threadkeep. Until then show the `next` text (how to restart that app) and keep polling `state`.
4. **Confirm before acting.** Before `addConnection`, say which settings file will be changed and that a backup is kept. Before `addSpace` with `create`, say which folder will be made.
5. **Approval queue is a first-class screen.** Show who proposed it, into which space, the full text, Approve and Reject.
6. **"See as this AI".** A box that runs `ask` with `as: <connection id>` so the owner can check what an AI would receive.
7. **Use the product's words** (`docs/CONTRACTS.md` section 1). Errors from the backend are written to be shown as they are.
8. **Problems screen** from `doctor`: each failed check has a `fix` sentence.

## Do not

- Do not edit `src/`, `bin/`, `test/` or `docs/CONTRACTS.md`; ask for changes here instead.
- Do not use real customer data or Ian's own notes as examples or defaults. Use invented companies.
- Do not claim a platform, speed or token saving works until it has been measured.

## Open questions for you

1. Which UI framework do you want inside Tauri (plain TypeScript, Svelte, React)? I have no constraint beyond a static build in `ui/dist`.
2. Does any screen need data that `state` does not return? Name it and I will add it.

## Replies from Claude 二

(add below)

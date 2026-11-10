# Threadkeep

Your own memory and work threads, shared safely with your AI apps.

You keep a folder of plain Markdown notes. Threadkeep lets the AI apps you choose find cited answers
in it, remember and correct facts, and carry on each other's unfinished work, within limits you set.
Your notes stay in your folder as ordinary files.

**Status: early prototype, not ready for customers.** See [what works and what does not](docs/STATUS.md).

## Try it (needs Node.js 22 or newer)

```bash
git clone https://github.com/iantsai886/threadkeep.git
```

```bash
cd threadkeep
```

```bash
node bin/threadkeep.mjs space add ~/Documents/MyNotes --create --kind personal
```

```bash
node bin/threadkeep.mjs connect claude-desktop --space MyNotes:propose
```

```bash
node bin/threadkeep.mjs doctor
```

Then fully quit and reopen the AI app, start a new chat and ask: "What can you do in Threadkeep?"
`doctor` reports the connection as verified once the app has really called Threadkeep.

Run `node bin/threadkeep.mjs help` for every command.

## How access works

| Level | The AI can |
|---|---|
| `read` | search, open cited sections, resume work |
| `propose` | also suggest memories, which wait for your approval (`threadkeep review`) |
| `write` | also store memories at once |

A space marked sensitive always needs your approval. A note with `private: true` in its header, and
any folder listed in `.threadkeepignore`, is never given to an AI.

## Documents

- [Status: done, not done, not verified](docs/STATUS.md)
- [Architecture and the Rust + Tauri decision](docs/ARCHITECTURE.md)
- [Shared contracts](docs/CONTRACTS.md)
- [Handoff to Claude 二](docs/HANDOFF-CLAUDE-2.md)

## Tests

```bash
npm test
```

Tests use invented data in temporary folders and remove them afterwards.

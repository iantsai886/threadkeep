# Threadkeep architecture and the Rust + Tauri decision

Date: 2026-11-10. Owner: Claude 一.

## Decision

Adopt a Rust core, a Tauri desktop shell and a TypeScript interface, for Windows, macOS and Linux.
The Node code in `src/` becomes the reference prototype and the executable specification. It is
replaced module by module, not thrown away in one step.

**Status: core and command line ported, shell not started.** `crates/core` (rules, owner commands,
connectors, backup) and `crates/cli` (`threadkeep` with every command, `mcp`, and a local `app`
server for the interface) build and pass their tests on Windows (see `docs/STATUS.md`). The Tauri
shell is not started: it needs the Visual Studio C++ Build Tools, which are not installed. Statements below
about installers, macOS and Linux are still unverified.

## Why the stack fits

| Need | Rust + Tauri | Node (current prototype) |
|---|---|---|
| What an AI app launches | One small native program, starts in milliseconds, no runtime to locate | Needs a bundled Node runtime and an absolute path to it |
| Installer size | System web view; typically a few megabytes | Bundled runtime; tens of megabytes per platform |
| Three platforms | One codebase; installers for `.msi`/`.exe`, `.dmg`, `.deb`/AppImage from Tauri's bundler | Three hand-written packaging scripts |
| Interface to core | In-process commands; no local web server, no session token, no open port | Loopback HTTP server with a token |
| Safety of the core | Memory-safe, strict types on the permission paths | Dynamic types |

Costs to accept:

- Each platform must be built on that platform (or in CI). A Mac is needed for the macOS build and for signing.
- Linux needs WebKitGTK present; behaviour varies between distributions and must be tested on a real desktop.
- Code signing is still required on Windows and macOS for a warning-free install.
- Compile times are longer, and a toolchain install of several gigabytes is needed on Windows.

## Does adopting it now cause significant rework?

No. The project is hours old.

| Part | Lines today | Fate |
|---|---|---|
| Contracts: file formats, permission rules, tool surface, command names (`docs/CONTRACTS.md`) | n/a | Kept unchanged |
| Tests (`test/`, 14 passing) | about 200 | Kept as the specification; the Rust core must pass the same cases |
| Core logic (`src/*.mjs`): storage, permissions, search, memories, threads, approvals, backup, connectors, MCP | about 1,300 | Rewritten in Rust, same module boundaries |
| Local HTTP server (`src/api.mjs`) | about 100 | Dropped; replaced by Tauri commands with the same names |
| Command line (`bin/`) | about 130 | Rewritten as the same Rust binary that serves MCP |

Nothing has been released, no customer data exists in this format, and the interface has not been
started, so there is nothing to migrate.

## Target layout

```
crates/
  core/        library: all rules and file handling (no UI, no network)
  cli/         the `threadkeep` program: owner commands and `threadkeep mcp` (what AI apps launch)
app/
  src-tauri/   Tauri shell: exposes core functions as commands named in CONTRACTS.md section 6
  ui/          TypeScript interface (Claude 二)
test/          cross-implementation cases
```

- AI apps launch the small `cli` program, never the desktop app, so a closed app does not break AI access.
- The desktop app and the CLI both link `core`; there is one implementation of every rule.
- Dependencies are kept few: `serde`/`serde_json`, `sha2`, `rand`, `toml_edit` for Codex settings, and Tauri. Search stays in-process (BM25 over Markdown sections); no database, no embedding model.

## Order of work once the toolchain is available

1. `crates/core` with storage, permissions, search; port the privacy tests first.
2. Memories, approvals, threads; port the memory tests.
3. `crates/cli` with `mcp`; rerun the end-to-end stdio check against the Rust binary.
4. Tauri shell exposing the commands; Claude 二's interface connects to it.
5. Installers on each platform; real install checks on Windows, then macOS and Linux.

## What is measured so far (Node prototype, fictional data)

- 14 of 14 tests pass on Windows, Node 24.
- End-to-end: a real stdio server took a checkpoint and returned it through `resume`.
- AI-facing surface: 6 tools, about 1,000 estimated tokens including instructions (characters / 4; not a provider-reported count).

Not measured: answer quality, token use in a real AI app, speed on a large space, any live AI app
connection, macOS, Linux.

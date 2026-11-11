#!/usr/bin/env node
// Threadkeep command line. Everything the product does is reachable from here.
import { spawn } from 'node:child_process';
import { appHome, loadConfig, PRODUCT, VERSION } from '../src/home.mjs';
import { connectionContext } from '../src/access.mjs';
import { createMcp, serveStdio } from '../src/mcp.mjs';
import { createOps } from '../src/ops.mjs';
import { serveApp } from '../src/api.mjs';
import { CLIENTS } from '../src/connectors.mjs';

const HELP = `${PRODUCT} ${VERSION} — your own memory and work threads, shared safely with your AI apps.

Set up
  threadkeep space add <folder> [--name N] [--kind personal|company|client] [--create] [--sensitive|--open]
  threadkeep space list | space set <name> [--kind K] [--sensitive|--open] | space remove <name>
  threadkeep connect <app> --space <name>[:read|propose|write] [--space ...] [--name N]
        apps: ${Object.keys(CLIENTS).join(', ')}, custom
  threadkeep connections | access <id> --space <name>[:level] ... | disconnect <id|app>
  threadkeep status            what is set up and whether each AI app has really connected
  threadkeep doctor            find problems and how to fix them

Daily use
  threadkeep review | approve <proposal> | reject <proposal>
  threadkeep ask "<question>" [--space S] [--as <connection>] [--history]
  threadkeep open "<ref>" [--as <connection>]
  threadkeep remember "<statement>" --space S --source "<where from>" [--kind K] [--replaces <id>]
  threadkeep memories [--space S] [--status active|superseded|retired]
  threadkeep retire <id> --space S | forget <id> --space S
  threadkeep threads [--space S] [--all]

Safety net
  threadkeep backup <space> <folder> | verify <backup-folder> | restore <backup-folder> <new-folder>
  threadkeep backups [folder]   backups in a folder (default: where the last backup went)
  threadkeep activity [--n 30]  tool names and outcomes only, never content
  threadkeep overhead           size of what every AI conversation carries

Run
  threadkeep app [--port P] [--no-open]   local app (127.0.0.1 only)
  threadkeep mcp                          what AI apps launch; not for people

Access levels: read = ask and resume; propose = may also suggest memories, which wait for your
approval; write = memories are stored at once. A sensitive space always needs your approval.
Settings folder: ${appHome()}  (override with THREADKEEP_HOME)`;

function parse(argv) {
  const pos = [], flags = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (!a.startsWith('--')) { pos.push(a); continue; }
    const key = a.slice(2);
    const next = argv[i + 1];
    const value = next === undefined || next.startsWith('--') ? true : (i++, next);
    if (key === 'space') (flags.space ||= []).push(value); else flags[key] = value;
  }
  return { pos, flags };
}

const grantsOf = (spaces) => (spaces || []).map((s) => { const m = String(s).match(/^(.*?)(?::(read|propose|write))?$/); return { space: m[1], level: m[2] || 'propose' }; });
const one = (flags) => (Array.isArray(flags.space) ? flags.space[0] : undefined);
const print = (v) => console.log(typeof v === 'string' ? v : JSON.stringify(v, null, 2));
const sensitiveFlag = (f) => (f.sensitive ? true : f.open ? false : undefined);

function openInBrowser(url) {
  const [cmd, args] = process.platform === 'win32' ? ['cmd', ['/c', 'start', '', url]] : process.platform === 'darwin' ? ['open', [url]] : ['xdg-open', [url]];
  try { spawn(cmd, args, { stdio: 'ignore', detached: true }).on('error', () => {}).unref(); } catch { /* the URL is printed anyway */ }
}

async function main() {
  const [cmd = 'help', ...rest] = process.argv.slice(2);
  const { pos, flags } = parse(rest);
  const home = appHome();
  const ops = createOps({ home });

  switch (cmd) {
    case 'help': case '--help': case '-h': return print(HELP);
    case 'version': case '--version': return print(VERSION);

    case 'mcp': {
      // Settings are re-read on every request: a revoked key stops working without a restart.
      const mcp = createMcp(async () => connectionContext(await loadConfig(home), home, process.env.THREADKEEP_TOKEN));
      await serveStdio(mcp);
      return;
    }
    case 'app': {
      const s = await serveApp(ops, { port: flags.port ? Number(flags.port) : 0 });
      print(`${PRODUCT} is running at ${s.url}\nOnly this computer can reach it. Press Ctrl+C to stop.`);
      if (!flags['no-open']) openInBrowser(s.url);
      await new Promise(() => {});
      return;
    }

    case 'space': {
      const [sub, a] = pos;
      if (sub === 'add') return print(await ops.addSpace({ path: a, name: flags.name, kind: flags.kind, create: Boolean(flags.create), sensitive: sensitiveFlag(flags) }));
      if (sub === 'set') return print(await ops.updateSpace({ name: a, kind: flags.kind === true ? undefined : flags.kind, sensitive: sensitiveFlag(flags) }));
      if (sub === 'remove') return print(await ops.removeSpace({ name: a }));
      return print((await ops.state()).spaces);
    }
    case 'connect': return print(await ops.addConnection({ client: pos[0], name: flags.name === true ? undefined : flags.name, grants: grantsOf(flags.space) }));
    case 'connections': return print(await ops.connections());
    case 'access': return print(await ops.setGrants({ id: pos[0], grants: grantsOf(flags.space) }));
    case 'disconnect': return print(await ops.revokeConnection({ id: pos[0] }));
    case 'status': { const s = await ops.state(); return print({ spaces: s.spaces, connections: s.connections, waiting_for_your_approval: s.pending }); }
    case 'doctor': { const d = await ops.doctor(); for (const c of d.checks) console.log(`${c.ok ? 'ok  ' : 'FIX '} ${c.what}${c.ok ? '' : `\n       -> ${c.fix}`}`); process.exitCode = d.ok ? 0 : 1; return; }

    case 'review': { const p = await ops.pending(); return print(p.length ? p.map((x) => ({ proposal: x.id, from: x.connection.name, space: x.space, type: x.type, summary: x.summary, created: x.created })) : 'Nothing is waiting for your approval.'); }
    case 'approve': return print(await ops.approve({ id: pos[0] }));
    case 'reject': return print(await ops.reject({ id: pos[0] }));

    case 'ask': return print(await ops.ask({ question: pos.join(' '), space: one(flags), as: flags.as, history: Boolean(flags.history), limit: flags.limit ? Number(flags.limit) : undefined }));
    case 'open': return print(await ops.open({ ref: pos.join(' '), as: flags.as }));
    case 'remember': return print(await ops.remember({ space: one(flags), statement: pos.join(' '), source: flags.source === true ? undefined : flags.source, kind: flags.kind, confidence: flags.confidence, replaces: flags.replaces, review_after: flags['review-after'] }));
    case 'memories': return print(await ops.memories({ space: one(flags), status: flags.status === true ? undefined : flags.status }));
    case 'retire': return print(await ops.retire({ space: one(flags), id: pos[0] }));
    case 'forget': return print(await ops.forget({ space: one(flags), id: pos[0] }));
    case 'threads': return print(await ops.threads({ space: one(flags), all: Boolean(flags.all) }));

    case 'backup': return print(await ops.backup({ space: pos[0], dest: pos[1] }));
    case 'backups': return print(await ops.listBackups({ dir: pos[0] }));
    case 'verify': return print(await ops.verifyBackup({ dir: pos[0] }));
    case 'restore': return print(await ops.restore({ dir: pos[0], target: pos[1] }));
    case 'activity': return print(await ops.activity({ n: flags.n }));
    case 'overhead': return print(ops.overhead());
    default: print(`Unknown command "${cmd}".\n\n${HELP}`); process.exitCode = 2;
  }
}

main().catch((e) => { console.error(`${PRODUCT}: ${e?.message || e}`); process.exitCode = 1; });

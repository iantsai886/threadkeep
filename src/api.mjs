// Local app server: the JSON API the interface (ui/) talks to, plus static files from ui/.
// Listens on 127.0.0.1 only, needs a per-launch token on every API call, and checks the Host
// header so another website cannot reach it through the browser. Contract: docs/CONTRACTS.md.
import { createServer } from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { readFile } from 'node:fs/promises';
import { randomBytes, timingSafeEqual } from 'node:crypto';

// The built interface. ui/ itself holds the TypeScript source, which a browser cannot load.
const UI_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'ui', 'dist');
const TYPES = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.mjs': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8', '.json': 'application/json', '.svg': 'image/svg+xml', '.png': 'image/png', '.ico': 'image/x-icon', '.woff2': 'font/woff2' };
const FALLBACK = '<!doctype html><meta charset="utf-8"><title>Threadkeep</title><body style="font-family:system-ui;max-width:40rem;margin:4rem auto;line-height:1.5"><h1>Threadkeep is running</h1><p>The interface files are not installed yet (no <code>ui/dist/index.html</code>; run <code>npm run build</code> in <code>ui</code>). Everything is available from the command line: run <code>threadkeep help</code>.</p>';

export function routes(ops) {
  const q = (url, k) => url.searchParams.get(k) || undefined;
  return {
    'GET /api/state': () => ops.state(),
    'GET /api/doctor': () => ops.doctor(),
    'GET /api/activity': (b, url) => ops.activity({ n: q(url, 'n') }),
    'GET /api/overhead': () => ops.overhead(),
    'POST /api/spaces': (b) => ops.addSpace(b),
    'POST /api/spaces/update': (b) => ops.updateSpace(b),
    'POST /api/spaces/remove': (b) => ops.removeSpace(b),
    'GET /api/connections': () => ops.connections(),
    'POST /api/connections': (b) => ops.addConnection(b),
    'POST /api/connections/grants': (b) => ops.setGrants(b),
    'POST /api/connections/revoke': (b) => ops.revokeConnection(b),
    'GET /api/pending': () => ops.pending(),
    'POST /api/pending/approve': (b) => ops.approve(b),
    'POST /api/pending/reject': (b) => ops.reject(b),
    'POST /api/ask': (b) => ops.ask(b),
    'POST /api/open': (b) => ops.open(b),
    'POST /api/preview': (b) => ops.preview(b),
    'GET /api/memories': (b, url) => ops.memories({ space: q(url, 'space'), status: q(url, 'status') }),
    'POST /api/memories': (b) => ops.remember(b),
    'POST /api/memories/retire': (b) => ops.retire(b),
    'POST /api/memories/forget': (b) => ops.forget(b),
    'GET /api/threads': (b, url) => ops.threads({ space: q(url, 'space'), all: q(url, 'all') === '1' }),
    'POST /api/threads/checkpoint': (b) => ops.checkpoint(b),
    'GET /api/backups': (b, url) => ops.listBackups({ dir: q(url, 'dir') }),
    'POST /api/backup': (b) => ops.backup(b),
    'POST /api/backup/verify': (b) => ops.verifyBackup(b),
    'POST /api/backup/restore': (b) => ops.restore(b),
  };
}

export async function serveApp(ops, { port = 0, token = randomBytes(24).toString('base64url'), uiDir = UI_DIR } = {}) {
  const table = routes(ops);
  const send = (res, code, body) => res.writeHead(code, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store', 'X-Content-Type-Options': 'nosniff' }).end(JSON.stringify(body));
  const server = createServer(async (req, res) => {
    try {
      const host = req.headers.host || '';
      if (host !== `127.0.0.1:${actual}` && host !== `localhost:${actual}`) { res.writeHead(421).end(); return; }
      const url = new URL(req.url, `http://${host}`);
      if (!url.pathname.startsWith('/api/')) {
        if (req.method !== 'GET') { res.writeHead(405).end(); return; }
        const rel = url.pathname === '/' ? '/index.html' : decodeURIComponent(url.pathname);
        const full = path.resolve(uiDir, '.' + rel);
        const type = TYPES[path.extname(full).toLowerCase()];
        const headers = { 'Cache-Control': 'no-store', 'X-Content-Type-Options': 'nosniff', 'X-Frame-Options': 'DENY', 'Referrer-Policy': 'no-referrer', 'Content-Security-Policy': "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; frame-ancestors 'none'" };
        if (!type || (full !== uiDir && !full.startsWith(uiDir + path.sep))) { res.writeHead(404, headers).end(); return; }
        try { res.writeHead(200, { ...headers, 'Content-Type': type }).end(await readFile(full)); }
        catch { if (rel === '/index.html') res.writeHead(200, { ...headers, 'Content-Type': TYPES['.html'] }).end(FALLBACK); else res.writeHead(404, headers).end(); }
        return;
      }
      // The token travels in a header only: never in an API URL, so it stays out of logs and history.
      const given = Buffer.from(String(req.headers['x-threadkeep-token'] || '')), want = Buffer.from(token);
      if (given.length !== want.length || !timingSafeEqual(given, want)) { send(res, 401, { error: 'This page has expired. Open Threadkeep again.' }); return; }
      const route = table[`${req.method} ${url.pathname}`];
      if (!route) { send(res, 404, { error: 'Unknown request.' }); return; }
      let body = {};
      if (req.method === 'POST') {
        if (!String(req.headers['content-type'] || '').startsWith('application/json')) { send(res, 415, { error: 'Send JSON.' }); return; }
        let raw = '';
        for await (const chunk of req) { raw += chunk; if (raw.length > 1_000_000) { send(res, 413, { error: 'Request too large.' }); return; } }
        try { body = raw ? JSON.parse(raw) : {}; } catch { send(res, 400, { error: 'That was not valid JSON.' }); return; }
        if (!body || typeof body !== 'object' || Array.isArray(body)) { send(res, 400, { error: 'Send a JSON object.' }); return; }
      }
      try { send(res, 200, { ok: true, result: await route(body, url) }); }
      catch (e) { send(res, 400, { ok: false, error: String(e?.message || e) }); }
    } catch { try { res.writeHead(500).end(); } catch { /* connection already gone */ } }
  });
  let actual = port;
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(port, '127.0.0.1', resolve); });
  actual = server.address().port;
  // The token reaches the page once, in the URL fragment, which browsers never send to a server.
  return { server, port: actual, token, url: `http://127.0.0.1:${actual}/#t=${token}`, close: () => new Promise((r) => server.close(r)) };
}

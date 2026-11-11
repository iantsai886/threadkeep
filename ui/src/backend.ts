// The only place that knows the transport.
// Today: the local server started by `threadkeep app`.
// Later: replace the body of call() with Tauri's invoke(command, args).

const DEMO = import.meta.env.VITE_DEMO === '1';
const TOKEN_KEY = 'threadkeep-token';

// The token arrives once in the page URL fragment (#t=...). Keep it for this tab
// only and take it out of the address bar.
function takeToken(): string {
  const m = location.hash.match(/^#t=([^&]+)/);
  if (m) {
    try {
      sessionStorage.setItem(TOKEN_KEY, m[1]);
    } catch {
      // storage blocked: the token still works until the page reloads
    }
    history.replaceState(null, '', location.pathname + location.search + '#/');
    return m[1];
  }
  try {
    return sessionStorage.getItem(TOKEN_KEY) ?? '';
  } catch {
    return '';
  }
}

let token = takeToken();
// Threadkeep was started again and its new address opened in this same tab.
window.addEventListener('hashchange', () => {
  if (!location.hash.startsWith('#t=')) return;
  token = takeToken();
  window.dispatchEvent(new Event('threadkeep-reopened'));
});

// Command name (docs/CONTRACTS.md section 6) to its route on the local server (src/api.mjs).
// GET routes take their arguments as query parameters.
const ROUTES: Record<string, [method: 'GET' | 'POST', path: string]> = {
  state: ['GET', 'state'],
  doctor: ['GET', 'doctor'],
  activity: ['GET', 'activity'],
  overhead: ['GET', 'overhead'],
  addSpace: ['POST', 'spaces'],
  updateSpace: ['POST', 'spaces/update'],
  removeSpace: ['POST', 'spaces/remove'],
  connections: ['GET', 'connections'],
  addConnection: ['POST', 'connections'],
  setGrants: ['POST', 'connections/grants'],
  revokeConnection: ['POST', 'connections/revoke'],
  pending: ['GET', 'pending'],
  approve: ['POST', 'pending/approve'],
  reject: ['POST', 'pending/reject'],
  ask: ['POST', 'ask'],
  open: ['POST', 'open'],
  preview: ['POST', 'preview'],
  memories: ['GET', 'memories'],
  remember: ['POST', 'memories'],
  retire: ['POST', 'memories/retire'],
  forget: ['POST', 'memories/forget'],
  threads: ['GET', 'threads'],
  checkpoint: ['POST', 'threads/checkpoint'],
  backup: ['POST', 'backup'],
  listBackups: ['GET', 'backups'],
  verifyBackup: ['POST', 'backup/verify'],
  restore: ['POST', 'backup/restore'],
};

function query(args: object): string {
  const q = new URLSearchParams();
  for (const [k, v] of Object.entries(args)) {
    if (v === undefined || v === null || v === false || v === '') continue;
    q.set(k, v === true ? '1' : String(v));
  }
  const s = q.toString();
  return s ? `?${s}` : '';
}

export async function call<T>(command: string, args: object = {}): Promise<T> {
  if (DEMO) return (await import('./demo')).demoCall(command, args) as Promise<T>;

  const route = ROUTES[command];
  if (!route) throw new Error(`Unknown command: ${command}.`);
  const [method, path] = route;

  let res: Response;
  try {
    res =
      method === 'GET'
        ? await fetch(`/api/${path}${query(args)}`, { headers: { 'x-threadkeep-token': token } })
        : await fetch(`/api/${path}`, {
            method: 'POST',
            headers: { 'content-type': 'application/json', 'x-threadkeep-token': token },
            body: JSON.stringify(args),
          });
  } catch {
    throw new Error('Threadkeep is not answering. Start it again, then reload this page.');
  }

  // The server answers { ok: true, result } or { ok: false, error }.
  // Its errors are plain sentences meant to be shown as they are.
  let body: { ok?: boolean; result?: T; error?: string } | null = null;
  try {
    body = await res.json();
  } catch {
    // no JSON body: handled below
  }
  if (!res.ok || !body || body.ok === false) {
    throw new Error(body?.error || `Threadkeep answered with an error (${res.status}).`);
  }
  return body.result as T;
}

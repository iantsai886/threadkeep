import { call } from '../backend';
import type { Check, Connection, Grant, Level, State } from './types';

export const app = $state<{ state: State | null; error: string; notice: string; checks: Check[]; setup: boolean | null }>({
  state: null,
  error: '',
  notice: '', // one sentence from the backend, shown on the next screen
  checks: [],
  setup: null, // null until the first answer; then whether first-run setup is showing
});

export const route = $state({ name: 'home', arg: '' });

export const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

function normalize(raw: any): State {
  return {
    spaces: raw?.spaces ?? [],
    connections: raw?.connections ?? [],
    pending: Number(raw?.pending ?? 0),
    apps: Object.values(raw?.apps ?? {}),
  };
}

export async function refresh(): Promise<void> {
  try {
    app.state = normalize(await call('state'));
    app.error = '';
    if (app.setup === null) app.setup = app.state.spaces.length === 0 && app.state.connections.length === 0;
  } catch (e) {
    app.error = message(e);
  }
}

export async function refreshChecks(): Promise<void> {
  try {
    app.checks = (await call<{ checks: Check[] }>('doctor')).checks ?? [];
  } catch {
    // The Problems screen reports this itself; the header count just stays as it was.
  }
}

// Show a sentence on the screen the owner lands on next; it goes away when they move on.
let keepNotice = false;
export function flash(sentence?: string): void {
  if (!sentence) return;
  app.notice = sentence;
  keepNotice = true;
}

export function startRouter(): () => void {
  const read = () => {
    if (keepNotice) keepNotice = false;
    else app.notice = '';
    const m = location.hash.match(/^#\/([a-z-]*)(?:\/(.*))?$/);
    route.name = m?.[1] || 'home';
    route.arg = m?.[2] ? decodeURIComponent(m[2]) : '';
    window.scrollTo(0, 0);
  };
  read();
  window.addEventListener('hashchange', read);
  return () => window.removeEventListener('hashchange', read);
}

export const link = (name: string, arg = '') => `#/${name}${arg ? '/' + encodeURIComponent(arg) : ''}`;
export const go = (name: string, arg = '') => {
  location.hash = link(name, arg);
};

// --- words shown to the customer ---

export type Choice = Level | 'none';

export const LEVELS: { value: Choice; label: string }[] = [
  { value: 'none', label: 'No access' },
  { value: 'read', label: 'Read' },
  { value: 'propose', label: 'Suggest (needs approval)' },
  { value: 'write', label: 'Save directly' },
];

export const KINDS = [
  { value: 'personal', label: 'Personal' },
  { value: 'company', label: 'Company' },
  { value: 'client', label: 'Client' },
] as const;

export const levelOf = (c: Connection, space: string): Choice => c.grants.find((g) => g.space === space)?.level ?? 'none';

export const levelLabel = (v: Choice) => LEVELS.find((l) => l.value === v)?.label ?? v;

export function withLevel(grants: Grant[], space: string, level: Choice): Grant[] {
  const rest = grants.filter((g) => g.space !== space);
  return level === 'none' ? rest : [...rest, { space, level }];
}

export const count = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;

export function size(bytes: number): string {
  if (bytes < 1024) return `${bytes} bytes`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export function when(iso?: string): string {
  if (!iso) return '';
  const d = new Date(iso);
  return isNaN(d.getTime()) ? iso : d.toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });
}

// JSON shapes of the owner commands (docs/CONTRACTS.md section 6), as returned by src/ops.mjs.

export type Level = 'read' | 'propose' | 'write';
export type Kind = 'personal' | 'company' | 'client';

export interface Space {
  name: string;
  path: string;
  kind: Kind;
  sensitive: boolean;
  exists?: boolean; // false when the folder cannot be reached
  notes?: number | null;
}

export interface Grant {
  space: string;
  level: Level;
}

export interface Connection {
  id: string;
  name: string;
  client: string;
  status: 'configured' | 'verified';
  next?: string;
  created?: string;
  last_call?: string | null;
  calls?: number;
  grants: Grant[];
}

// One entry of state.apps: an AI app Threadkeep knows how to set up.
export interface KnownApp {
  id: string;
  title: string;
  file: string; // the settings file addConnection changes
  present: boolean; // installed on this computer
  configured: boolean;
  reload: string;
}

export interface State {
  spaces: Space[];
  connections: Connection[];
  pending: number;
  apps: KnownApp[]; // state.apps is keyed by id on the wire; the store turns it into a list
}

export interface Check {
  ok: boolean;
  what: string;
  fix?: string;
}

export interface Proposal {
  id: string;
  type: 'memory' | 'checkpoint';
  space: string;
  created: string;
  connection: { id: string; name: string };
  summary: string;
  payload: Record<string, unknown>;
}

export interface Source {
  ref: string;
  space: string;
  kind: 'note' | 'memory';
  title?: string;
  heading?: string;
  date?: string;
  id?: string;
  confidence?: string;
  outdated?: boolean;
  replaced_by?: string;
  needs_review?: boolean;
  excerpt: string;
}

export interface AskResult {
  status: 'found' | 'partial' | 'none';
  covered: string[];
  missing: string[];
  more_available?: boolean;
  sources: Source[];
  est_tokens?: number;
}

export interface Opened {
  ref: string;
  text: string;
  truncated?: boolean;
}

export interface Memory {
  id: string;
  kind: string;
  status: 'active' | 'superseded' | 'retired';
  confidence?: string;
  source?: string;
  created?: string;
  by?: string;
  supersedes?: string;
  superseded_by?: string;
  review_after?: string;
  due?: boolean; // review_after has passed
  statement: string;
}

export interface RememberResult {
  saved: boolean;
  id?: string;
  duplicate_of?: string;
}

export interface ForgetResult {
  forgotten: string;
  not_removed?: string[];
}

export interface Thread {
  space: string;
  thread: string;
  title: string;
  status: 'open' | 'done';
  goal?: string;
  state?: string;
  decisions?: string[];
  sources?: string[];
  open_questions?: string[];
  next_action?: string;
  last_checkpoint?: { by: string; at: string };
  checkpoints?: number;
}

export interface AddConnectionResult {
  id: string;
  name: string;
  next?: string;
  verify?: string;
  // a known app: its settings file was changed
  file?: string;
  backup?: string | null;
  // custom: shown once
  token?: string;
  snippet?: string;
}

export interface RevokeResult {
  revoked: string;
  settings_entry_removed?: boolean;
  warning?: string;
  note?: string;
}

export interface RemoveSpaceResult {
  removed: string;
  folder_kept?: string;
  note?: string;
}

export interface BackupResult {
  backup: string; // the folder that was created
  files: number;
  bytes: number;
  verified: boolean;
}

// One AI call. Never holds a question, a statement or note text.
export interface ActivityEntry {
  at: string;
  tool: string;
  connection: string | null;
  name: string | null;
  ok: boolean;
  ms?: number;
  est_tokens?: number;
  status?: string; // for ask: found, partial, none
  queued?: boolean; // waiting for approval
}

export interface Overhead {
  tools: number;
  total_est_tokens: number;
  method: string;
}

export interface BackupEntry {
  dir: string; // pass to verifyBackup and restore
  space: string;
  kind?: string;
  created: string;
  files: number;
  bytes: number;
}

export interface BackupList {
  dir: string | null;
  remembered: boolean; // the folder of the last backup was used
  backups: BackupEntry[];
}

export interface BackupCheck {
  ok: boolean;
  problems: string[];
  space?: string;
  created?: string;
  files?: number;
}

export interface RestoreResult {
  restored_to: string;
  files: number;
  space: string;
  next: string;
}

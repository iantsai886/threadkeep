// Who may do what. One rule set, applied before anything is read:
//   - A connection is one AI app with its own key. No key, unknown key or revoked key: nothing.
//   - A connection sees only the spaces it was granted, at one level per space:
//       read     ask, open, resume
//       propose  + save and checkpoint; saved memories wait for the owner's approval
//       write    + saved memories are stored at once
//   - A space marked sensitive never accepts a direct write from an AI, whatever the level.
import { createHash, randomBytes, timingSafeEqual } from 'node:crypto';
import { LEVELS } from './home.mjs';

export const hashToken = (token) => createHash('sha256').update(String(token)).digest('hex');
export const newToken = () => `tk_${randomBytes(24).toString('hex')}`;
export const newConnectionId = () => `c-${randomBytes(4).toString('hex')}`;

const rank = (level) => LEVELS.indexOf(level);
export const atLeast = (level, wanted) => rank(level) >= rank(wanted) && rank(level) >= 0;

export function findConnection(cfg, token) {
  if (typeof token !== 'string' || token.length < 20) return null;
  const want = Buffer.from(hashToken(token), 'hex');
  for (const c of cfg.connections) {
    if (c.revoked) continue;
    const have = Buffer.from(c.tokenHash, 'hex');
    if (have.length === want.length && timingSafeEqual(have, want)) return c;
  }
  return null;
}

const findSpace = (cfg, name) => cfg.spaces.find((s) => s.name.toLowerCase() === String(name || '').toLowerCase());

function grantedSpaces(cfg, connection) {
  return connection.grants.map((g) => { const s = findSpace(cfg, g.space); return s ? { ...s, level: g.level } : null; }).filter(Boolean);
}

// Context for an AI connection. Unknown callers get an empty one, never a default.
export function connectionContext(cfg, home, token) {
  const connection = findConnection(cfg, token);
  return { cfg, home, owner: false, connection, spaces: connection ? grantedSpaces(cfg, connection) : [] };
}

// Context for the owner at the keyboard (CLI and the local app).
export function ownerContext(cfg, home) {
  return { cfg, home, owner: true, connection: null, spaces: cfg.spaces.map((s) => ({ ...s, level: 'write' })) };
}

// "See as this AI": the owner previews exactly what one connection would get.
export function previewContext(cfg, home, connectionId) {
  const connection = cfg.connections.find((c) => c.id === connectionId && !c.revoked);
  if (!connection) throw new Error('Unknown connection.');
  return { cfg, home, owner: false, connection, spaces: grantedSpaces(cfg, connection), preview: true };
}

export function spaceFor(ctx, name) {
  const s = ctx.spaces.find((x) => x.name.toLowerCase() === String(name || '').toLowerCase());
  // Same message whether the space does not exist or is not granted: do not confirm its existence.
  if (!s) throw new Error(`No space named "${name}" is available here. Available: ${ctx.spaces.map((x) => x.name).join(', ') || 'none'}.`);
  return s;
}

// Where a write goes when the caller did not name a space: only an unambiguous choice is accepted.
export function writableSpace(ctx, name) {
  if (name) {
    const s = spaceFor(ctx, name);
    if (!atLeast(s.level, 'propose')) throw new Error(`This connection can read "${s.name}" but not add to it.`);
    return s;
  }
  const options = ctx.spaces.filter((s) => atLeast(s.level, 'propose'));
  if (options.length === 1) return options[0];
  if (!options.length) throw new Error('This connection is read-only.');
  throw new Error(`Say which space this belongs to: ${options.map((s) => s.name).join(', ')}.`);
}

// true = store now; false = queue for the owner's approval.
export const writesDirectly = (ctx, space) => ctx.owner || (space.level === 'write' && !space.sensitive);
// Checkpoints are working notes for handover, so "propose" may store them, except in sensitive spaces.
export const checkpointsDirectly = (ctx, space) => ctx.owner || (atLeast(space.level, 'propose') && !space.sensitive);

export function describeAccess(ctx) {
  return ctx.spaces.map((s) => ({
    space: s.name, kind: s.kind, sensitive: s.sensitive, level: s.level,
    can: ['ask', 'open', 'resume', ...(atLeast(s.level, 'propose') ? ['save', 'checkpoint'] : [])],
    save: !atLeast(s.level, 'propose') ? 'not allowed' : writesDirectly(ctx, s) ? 'stored at once' : 'waits for owner approval',
    checkpoint: !atLeast(s.level, 'propose') ? 'not allowed' : checkpointsDirectly(ctx, s) ? 'stored at once' : 'waits for owner approval',
  }));
}

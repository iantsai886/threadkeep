// Every test works in its own temp folder and removes it afterwards. Fictional data only.
import os from 'node:os';
import path from 'node:path';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { after } from 'node:test';
import { createOps } from '../src/ops.mjs';
import { loadConfig } from '../src/home.mjs';
import { connectionContext } from '../src/access.mjs';
import { createMcp } from '../src/mcp.mjs';
import { clearSearchCache } from '../src/search.mjs';

const made = [];
after(async () => { clearSearchCache(); for (const d of made) await rm(d, { recursive: true, force: true, maxRetries: 5, retryDelay: 50 }); });

export async function tmp() {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'threadkeep-test-'));
  made.push(dir);
  return dir;
}

export async function write(root, rel, text) {
  const full = path.join(root, ...rel.split('/'));
  await mkdir(path.dirname(full), { recursive: true });
  await writeFile(full, text);
  return full;
}

// A fictional company space and a fictional personal space, with planted private material.
export async function world() {
  const base = await tmp();
  const home = path.join(base, 'app'), userHome = path.join(base, 'user');
  const company = path.join(base, 'Northwind'), personal = path.join(base, 'Mira');
  await mkdir(userHome, { recursive: true });
  await write(company, 'Clients/Riverbend.md', '# Riverbend Co.\n\n## Contract\n\nThe Riverbend support contract renews every March. The account lead is Dana Whitfield.\n\n## Pricing\n\nRiverbend pays a fixed monthly retainer for support.\n');
  await write(company, 'Playbook/Onboarding.md', '# Onboarding playbook\n\nEvery new client gets a kickoff call in week one and a written scope in week two.\n');
  await write(company, 'Finance/Salaries.md', '---\nprivate: true\n---\n# Salaries\n\nCOMPANY_PRIVATE_CANARY Dana earns a confidential amount.\n');
  await write(company, 'Broken.md', '---\ntitle: Broken header\nprivate: true\n\n# Never closed\n\nMALFORMED_CANARY should stay hidden.\n');
  await write(company, 'HR/Reviews.md', '# Reviews\n\nIGNORED_FOLDER_CANARY performance notes.\n');
  await write(company, '.threadkeepignore', '# folders no AI may see\nHR\n');
  await write(personal, 'Health.md', '# Health\n\nPERSONAL_CANARY Mira prefers morning appointments.\n');
  const ops = createOps({ home, userHome, env: {}, platform: process.platform, launch: { command: process.execPath, args: ['threadkeep.mjs', 'mcp'] } });
  await ops.addSpace({ path: company, name: 'Northwind', kind: 'company' });
  await ops.addSpace({ path: personal, name: 'Mira', kind: 'personal' });
  return { base, home, userHome, company, personal, ops };
}

// Connect a fictional AI app and return a function that calls its tools the way the app would.
export async function connectAi(w, grants, name = 'Test AI') {
  const c = await w.ops.addConnection({ client: 'custom', name, grants });
  const mcp = createMcp(async () => connectionContext(await loadConfig(w.home), w.home, c.token));
  const call = async (tool, args = {}) => {
    const r = await mcp.handle({ jsonrpc: '2.0', id: 1, method: 'tools/call', params: { name: tool, arguments: args } });
    return { error: r.result.isError ? r.result.content[0].text : null, data: r.result.structuredContent, text: r.result.content[0].text };
  };
  return { ...c, mcp, call };
}

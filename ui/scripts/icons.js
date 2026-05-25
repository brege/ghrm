/**
 * Verify and generate the browser icon sprite contract.
 */

import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, '../..');
const sourcePath = join(__dirname, '../icons.json');
const spritePath = join(repoRoot, 'assets/templates/fragments/icons.html');
const refRoots = ['assets/templates', 'assets/css', 'src', 'ui/src'];
const skippedDirs = new Set(['.asset-check', '.vite-check', 'node_modules']);
const expectedIconCount = 56;
const dynamicAskamaContracts = [
  {
    expression: 'row.icon',
    template: 'assets/templates/fragments/about.html',
    provider: 'src/http/about.rs',
  },
  {
    expression: 'sort_dir_control.icon',
    template: 'assets/templates/fragments/explorer/header.html',
    provider: 'src/explorer.rs',
  },
];

function fail(message, ...parts) {
  console.error(message, ...parts);
  process.exit(1);
}

function toPosix(path) {
  return path.split(sep).join('/');
}

function readText(path) {
  try {
    return readFileSync(path, 'utf8');
  } catch (error) {
    fail('Unable to read file:', path, error.message);
  }
}

function writeText(path, text) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text);
}

function readSource() {
  let data;
  try {
    data = JSON.parse(readText(sourcePath));
  } catch (error) {
    fail('Unable to parse icon source:', error.message);
  }

  if (data.version !== 1) {
    fail('Unsupported icon source version:', String(data.version));
  }
  if (!Array.isArray(data.icons)) {
    fail('Icon source must contain an icons array.');
  }
  if (data.icons.length !== expectedIconCount) {
    fail(
      'Icon source has unexpected icon count:',
      `${data.icons.length}, expected ${expectedIconCount}`,
    );
  }

  const ids = new Set();
  for (const icon of data.icons) {
    if (typeof icon.id !== 'string' || !icon.id.startsWith('ghrm-icon-')) {
      fail('Invalid icon id:', String(icon.id));
    }
    if (typeof icon.symbol !== 'string' || !icon.symbol.includes('<symbol ')) {
      fail('Icon entry is missing symbol markup:', icon.id);
    }
    if (!icon.symbol.includes(`id="${icon.id}"`)) {
      fail('Icon symbol id does not match source id:', icon.id);
    }
    if (ids.has(icon.id)) {
      fail('Duplicate icon id:', icon.id);
    }
    ids.add(icon.id);
  }

  return data;
}

function renderSprite(data) {
  return `${data.sprite.open}\n${data.icons.map((icon) => icon.symbol).join('\n')}\n${data.sprite.close}\n`;
}

function walkFiles(dir) {
  const files = [];
  if (!existsSync(dir)) return files;

  for (const name of readdirSync(dir)) {
    if (skippedDirs.has(name)) continue;

    const path = join(dir, name);
    const stat = statSync(path);
    if (stat.isDirectory()) {
      files.push(...walkFiles(path));
    } else {
      files.push(path);
    }
  }

  return files;
}

function collectIconRefs() {
  const refs = new Map();
  const dynamicRefs = [];

  // Matches literal ghrm sprite IDs in Rust, templates, CSS, and TS.
  const iconRefPattern = /ghrm-icon-[A-Za-z0-9-]+/g;

  // Matches TS helper calls that expand to ghrm-icon-* at runtime.
  const tsIconCallPattern = /\bicon\('([A-Za-z0-9-]+)'/g;

  // Matches Rust alert icon branches that expand to ghrm-icon-* at runtime.
  const rustAlertPattern =
    /^\s*"([A-Za-z0-9-]+)"\s*=>\s*Some\("([A-Za-z0-9-]+)"\),/gm;

  // Matches Askama icon expressions whose concrete IDs are supplied by Rust.
  const askamaIconPattern = /href="#\{\{\s*([A-Za-z0-9_.]+)\s*\}\}"/g;

  function addRef(id, file, kind) {
    if (!refs.has(id)) refs.set(id, new Set());
    refs.get(id).add(`${toPosix(relative(repoRoot, file))} [${kind}]`);
  }

  for (const root of refRoots) {
    for (const file of walkFiles(join(repoRoot, root))) {
      if (file === spritePath || file === sourcePath) continue;

      const text = readText(file);
      for (const match of text.matchAll(iconRefPattern)) {
        addRef(match[0], file, 'literal');
      }
      for (const match of text.matchAll(tsIconCallPattern)) {
        addRef(`ghrm-icon-${match[1]}`, file, 'ts-helper');
      }
      if (file.endsWith('src/render/alert.rs')) {
        for (const match of text.matchAll(rustAlertPattern)) {
          addRef(`ghrm-icon-${match[2]}`, file, 'rust-alert');
        }
      }
      for (const match of text.matchAll(askamaIconPattern)) {
        dynamicRefs.push({
          expression: match[1],
          file: toPosix(relative(repoRoot, file)),
        });
      }
    }
  }

  return { refs, dynamicRefs };
}

function sourceIds(data) {
  return new Set(data.icons.map((icon) => icon.id));
}

function refsByProvider(refs, provider) {
  const matches = [];
  for (const [id, files] of refs) {
    for (const file of files) {
      if (file.startsWith(`${provider} `)) {
        matches.push(id);
      }
    }
  }
  return [...new Set(matches)].sort();
}

function checkDynamicRefs(refs, dynamicRefs) {
  const missing = [];

  for (const contract of dynamicAskamaContracts) {
    const hasTemplateUse = dynamicRefs.some(
      (ref) =>
        ref.file === contract.template &&
        ref.expression === contract.expression,
    );
    if (!hasTemplateUse) {
      missing.push(
        `${contract.template} missing dynamic icon expression ${contract.expression}`,
      );
      continue;
    }

    const providerIds = refsByProvider(refs, contract.provider);
    if (providerIds.length === 0) {
      missing.push(`${contract.provider} does not provide icon IDs`);
    }
  }

  if (missing.length > 0) {
    fail('Dynamic icon contracts are not covered:', missing.join('\n'));
  }
}

function checkRefs(data, refs, dynamicRefs) {
  const iconIds = sourceIds(data);
  const refIds = new Set(refs.keys());
  const missing = [];
  const unreferenced = [];

  for (const id of refIds) {
    if (!iconIds.has(id)) {
      missing.push(
        `${id} referenced by ${[...refs.get(id)].sort().join(', ')}`,
      );
    }
  }

  for (const id of iconIds) {
    if (!refIds.has(id)) {
      unreferenced.push(id);
    }
  }

  if (unreferenced.length > 0) {
    fail('Icon source contains unreferenced IDs:', unreferenced.join(', '));
  }

  if (missing.length > 0) {
    fail('Icon source is missing referenced IDs:', missing.join('\n'));
  }

  checkDynamicRefs(refs, dynamicRefs);
}

function reportRefs(refs, dynamicRefs) {
  console.log('Icon reference report:');
  for (const id of [...refs.keys()].sort()) {
    console.log(`- ${id}: ${[...refs.get(id)].sort().join(', ')}`);
  }
  console.log('Dynamic icon contracts:');
  for (const contract of dynamicAskamaContracts) {
    const providerIds = refsByProvider(refs, contract.provider);
    console.log(
      `- ${contract.template} ${contract.expression}: ${contract.provider} provides ${providerIds.join(', ')}`,
    );
  }
  for (const ref of dynamicRefs) {
    const known = dynamicAskamaContracts.some(
      (contract) =>
        contract.template === ref.file &&
        contract.expression === ref.expression,
    );
    if (!known) {
      console.log(`- ${ref.file} ${ref.expression}: unclassified`);
    }
  }
}

function checkGenerated(data) {
  const expected = renderSprite(data);
  const actual = readText(spritePath);

  if (actual !== expected) {
    fail(
      'Icon sprite differs from generated output. Run npm --prefix ui run icons:write.',
    );
  }
}

function writeGenerated(data) {
  writeText(spritePath, renderSprite(data));
}

const mode = process.argv[2] ?? 'check';
if (mode === 'write') {
  writeGenerated(readSource());
} else if (mode === 'check') {
  const data = readSource();
  const { refs, dynamicRefs } = collectIconRefs();
  checkRefs(data, refs, dynamicRefs);
  checkGenerated(data);
  reportRefs(refs, dynamicRefs);
  console.log('Icon sprite check passed.');
} else {
  fail('Unknown icon mode:', mode);
}

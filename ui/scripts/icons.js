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

  // Matches literal ghrm sprite IDs in Rust, templates, CSS, and TS.
  const iconRefPattern = /ghrm-icon-[A-Za-z0-9-]+/g;

  // Matches TS helper calls that expand to ghrm-icon-* at runtime.
  const tsIconCallPattern = /\bicon\('([A-Za-z0-9-]+)'/g;

  // Matches Rust alert icon branches that expand to ghrm-icon-* at runtime.
  const rustAlertPattern = /Some\("([A-Za-z0-9-]+)"\)/g;

  function addRef(id, file) {
    if (!refs.has(id)) refs.set(id, new Set());
    refs.get(id).add(toPosix(relative(repoRoot, file)));
  }

  for (const root of refRoots) {
    for (const file of walkFiles(join(repoRoot, root))) {
      if (file === spritePath || file === sourcePath) continue;

      const text = readText(file);
      for (const match of text.matchAll(iconRefPattern)) {
        addRef(match[0], file);
      }
      for (const match of text.matchAll(tsIconCallPattern)) {
        addRef(`ghrm-icon-${match[1]}`, file);
      }
      if (file.endsWith('src/render/alert.rs')) {
        for (const match of text.matchAll(rustAlertPattern)) {
          addRef(`ghrm-icon-${match[1]}`, file);
        }
      }
    }
  }

  return refs;
}

function checkRefs(data) {
  const iconIds = new Set(data.icons.map((icon) => icon.id));
  const missing = [];

  for (const [id, files] of collectIconRefs()) {
    if (!iconIds.has(id)) {
      missing.push(`${id} referenced by ${[...files].sort().join(', ')}`);
    }
  }

  if (missing.length > 0) {
    fail('Icon source is missing referenced IDs:', missing.join('\n'));
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
  checkRefs(data);
  checkGenerated(data);
  console.log('Icon sprite check passed.');
} else {
  fail('Unknown icon mode:', mode);
}

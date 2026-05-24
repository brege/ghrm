/**
 * Verify and package generated browser runtime assets.
 */

import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { dirname, join, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const assetsDir = join(__dirname, '../../assets');
const runtimeDir = join(assetsDir, 'js');
const archivePath = join(assetsDir, 'js.tar.zst');
const manifestPath = join(assetsDir, 'js.sha256.json');
const buildDir = join(__dirname, '../.vite-check');
const extractDir = join(__dirname, '../.asset-check');
const srcDir = join(__dirname, '../src');
const expectedEntries = ['preview.js', 'main.js', 'gist.js'];
const expectedChunks = ['adapters.js', 'explorer.js', 'shared.js'];

function fail(message, ...parts) {
  console.error(message, ...parts);
  process.exit(1);
}

function run(command, args) {
  const result = spawnSync(command, args, { stdio: 'inherit' });
  if (result.error) {
    fail(`${command} failed:`, result.error.message);
  }
  if (result.status !== 0) {
    fail(`${command} failed with status ${result.status}`);
  }
}

function resetDir(dir) {
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
}

function toPosix(path) {
  return path.split(sep).join('/');
}

function collectFiles(dir, base = dir) {
  if (!existsSync(dir)) {
    fail('Directory not found:', dir);
  }

  const result = [];
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    const stat = statSync(path);
    if (stat.isDirectory()) {
      result.push(...collectFiles(path, base));
    } else {
      result.push(toPosix(relative(base, path)));
    }
  }
  return result.sort();
}

function checkExpectedFiles(dir) {
  const files = collectFiles(dir);
  const fileSet = new Set(files);

  const missingEntries = expectedEntries.filter((entry) => !fileSet.has(entry));
  if (missingEntries.length > 0) {
    fail('Missing expected entry files:', missingEntries.join(', '));
  }

  const missingChunks = expectedChunks
    .map((chunk) => `chunks/${chunk}`)
    .filter((chunk) => !fileSet.has(chunk));
  if (missingChunks.length > 0) {
    fail('Missing expected chunk files:', missingChunks.join(', '));
  }
}

function checkNoSourceJs() {
  const sourceJs = collectFiles(srcDir).filter((file) => file.endsWith('.js'));
  if (sourceJs.length > 0) {
    fail('JavaScript files remain under ui/src:', sourceJs.join(', '));
  }
}

function sha256File(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

function manifestFor(dir) {
  const files = {};
  for (const file of collectFiles(dir)) {
    files[`js/${file}`] = sha256File(join(dir, file));
  }

  return {
    version: 1,
    algorithm: 'sha256',
    archive: {
      path: 'js.tar.zst',
      sha256: sha256File(archivePath),
    },
    files,
  };
}

function readManifest() {
  let manifest;
  try {
    manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
  } catch (error) {
    fail('Unable to read asset manifest:', error.message);
  }

  if (manifest.version !== 1) {
    fail('Unsupported asset manifest version:', String(manifest.version));
  }
  if (manifest.algorithm !== 'sha256') {
    fail('Unsupported asset manifest algorithm:', String(manifest.algorithm));
  }
  if (manifest.archive?.path !== 'js.tar.zst') {
    fail('Unexpected asset archive path:', String(manifest.archive?.path));
  }
  if (!manifest.files || typeof manifest.files !== 'object') {
    fail('Asset manifest missing files map.');
  }
  return manifest;
}

function packArchive() {
  if (!existsSync(runtimeDir)) {
    fail('Runtime asset directory not found:', runtimeDir);
  }
  run('tar', [
    '--create',
    '--zstd',
    '--file',
    archivePath,
    '--sort=name',
    '--mtime=@0',
    '--owner=0',
    '--group=0',
    '--numeric-owner',
    '--directory',
    assetsDir,
    'js',
  ]);
}

function extractArchive() {
  resetDir(extractDir);
  run('tar', [
    '--extract',
    '--zstd',
    '--file',
    archivePath,
    '--directory',
    extractDir,
  ]);
}

function compareLists(label, left, right) {
  const leftSet = new Set(left);
  const rightSet = new Set(right);
  const missing = right.filter((item) => !leftSet.has(item));
  const extra = left.filter((item) => !rightSet.has(item));

  if (missing.length > 0 || extra.length > 0) {
    if (missing.length > 0) {
      console.error(`${label} missing:`, missing.join(', '));
    }
    if (extra.length > 0) {
      console.error(`${label} extra:`, extra.join(', '));
    }
    process.exit(1);
  }
}

function checkManifest(manifest) {
  if (!existsSync(archivePath)) {
    fail('Asset archive not found:', archivePath);
  }
  const archiveHash = sha256File(archivePath);
  if (manifest.archive.sha256 !== archiveHash) {
    fail('Asset archive hash differs from manifest.');
  }

  const extractedJs = join(extractDir, 'js');
  const extractedFiles = collectFiles(extractedJs).map((file) => `js/${file}`);
  const manifestFiles = Object.keys(manifest.files).sort();
  compareLists('Asset manifest file list', extractedFiles, manifestFiles);

  for (const file of extractedFiles) {
    const extractedPath = join(extractDir, file);
    const hash = sha256File(extractedPath);
    if (manifest.files[file] !== hash) {
      fail('Asset file hash differs from manifest:', file);
    }
  }
}

function compareDirectories(leftDir, rightDir, leftName, rightName) {
  const leftFiles = collectFiles(leftDir);
  const rightFiles = collectFiles(rightDir);
  compareLists(`${leftName} vs ${rightName}`, leftFiles, rightFiles);

  const different = [];
  for (const file of leftFiles) {
    const leftContent = readFileSync(join(leftDir, file));
    const rightContent = readFileSync(join(rightDir, file));
    if (!leftContent.equals(rightContent)) {
      different.push(file);
    }
  }

  if (different.length > 0) {
    fail(`${leftName} differs from ${rightName}:`, different.join(', '));
  }
}

function calculateDirSize(dir) {
  let total = 0;
  for (const file of readdirSync(dir)) {
    const path = join(dir, file);
    const stat = statSync(path);
    if (stat.isDirectory()) {
      total += calculateDirSize(path);
    } else {
      total += stat.size;
    }
  }
  return total;
}

function formatBytes(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

function packBuild() {
  checkNoSourceJs();
  checkExpectedFiles(runtimeDir);
  packArchive();
  writeFileSync(
    manifestPath,
    `${JSON.stringify(manifestFor(runtimeDir), null, 2)}\n`,
  );

  console.log('Asset archive packed.');
  console.log('Entry files:', expectedEntries.join(', '));
  console.log('Chunks:', expectedChunks.join(', '));
  console.log('Total output size:', formatBytes(calculateDirSize(runtimeDir)));
  console.log('Archive size:', formatBytes(statSync(archivePath).size));
}

function checkBuild() {
  checkNoSourceJs();
  checkExpectedFiles(buildDir);
  extractArchive();

  const manifest = readManifest();
  checkManifest(manifest);
  compareDirectories(
    buildDir,
    join(extractDir, 'js'),
    'Build output',
    'asset archive',
  );

  console.log('Build check passed.');
  console.log('Entry files:', expectedEntries.join(', '));
  console.log('Chunks:', expectedChunks.join(', '));
  console.log('Total output size:', formatBytes(calculateDirSize(buildDir)));
}

function checkSourceBuild() {
  checkNoSourceJs();
  checkExpectedFiles(buildDir);

  console.log('Source build check passed.');
  console.log('Entry files:', expectedEntries.join(', '));
  console.log('Chunks:', expectedChunks.join(', '));
  console.log('Total output size:', formatBytes(calculateDirSize(buildDir)));
}

const mode = process.argv[2] ?? 'check';
if (mode === 'pack') {
  packBuild();
} else if (mode === 'source') {
  checkSourceBuild();
} else if (mode === 'check') {
  checkBuild();
} else {
  fail('Unknown asset check mode:', mode);
}

/**
 * Verify that Vite build output matches tracked runtime assets.
 * Fails on missing, extra, or byte-different generated files.
 */

import { readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const buildDir = join(__dirname, '../.vite-check');
const trackedDir = join(__dirname, '../../assets/js');
const expectedEntries = ['preview.js', 'main.js', 'gist.js'];
const expectedChunks = ['adapters.js', 'explorer.js', 'shared.js'];

function collectFiles(dir, base = dir) {
  const result = [];
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    const stat = statSync(path);
    if (stat.isDirectory()) {
      result.push(...collectFiles(path, base));
    } else {
      result.push(relative(base, path));
    }
  }
  return result.sort();
}

function checkExpectedFiles() {
  let buildFiles;
  try {
    buildFiles = readdirSync(buildDir);
  } catch {
    console.error('Build output directory not found:', buildDir);
    process.exit(1);
  }

  const missingEntries = expectedEntries.filter((e) => !buildFiles.includes(e));
  if (missingEntries.length > 0) {
    console.error('Missing expected entry files:', missingEntries.join(', '));
    process.exit(1);
  }

  const chunksDir = join(buildDir, 'chunks');
  let chunkFiles;
  try {
    chunkFiles = readdirSync(chunksDir);
  } catch {
    console.error('Build output chunks directory not found:', chunksDir);
    process.exit(1);
  }

  const missingChunks = expectedChunks.filter((c) => !chunkFiles.includes(c));
  if (missingChunks.length > 0) {
    console.error('Missing expected chunk files:', missingChunks.join(', '));
    process.exit(1);
  }
}

function compareDirectories() {
  const buildFiles = collectFiles(buildDir);
  const trackedFiles = collectFiles(trackedDir);

  const buildSet = new Set(buildFiles);
  const trackedSet = new Set(trackedFiles);

  const missing = trackedFiles.filter((f) => !buildSet.has(f));
  const extra = buildFiles.filter((f) => !trackedSet.has(f));

  if (missing.length > 0) {
    console.error(
      'Files in tracked assets/js but not in build:',
      missing.join(', '),
    );
    process.exit(1);
  }

  if (extra.length > 0) {
    console.error(
      'Files in build but not in tracked assets/js:',
      extra.join(', '),
    );
    process.exit(1);
  }

  const different = [];
  for (const file of buildFiles) {
    const buildContent = readFileSync(join(buildDir, file));
    const trackedContent = readFileSync(join(trackedDir, file));
    if (!buildContent.equals(trackedContent)) {
      different.push(file);
    }
  }

  if (different.length > 0) {
    console.error('Files differ from tracked assets/js:', different.join(', '));
    process.exit(1);
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

function checkBuild() {
  checkExpectedFiles();
  compareDirectories();

  console.log('Build check passed.');
  console.log('Entry files:', expectedEntries.join(', '));
  console.log('Chunks:', expectedChunks.join(', '));

  const totalSize = calculateDirSize(buildDir);
  console.log('Total output size:', formatBytes(totalSize));
}

checkBuild();

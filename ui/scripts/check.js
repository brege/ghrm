/**
 * Verify that Vite build output matches expected entry files.
 * This proves the build works without modifying tracked runtime assets.
 */

import { readdirSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const buildDir = join(__dirname, '../.vite-check');
const expectedEntries = ['preview.js', 'main.js', 'gist.js'];
const expectedChunks = ['adapters.js', 'explorer.js', 'shared.js'];

function checkBuild() {
  let files;
  try {
    files = readdirSync(buildDir);
  } catch {
    console.error('Build output directory not found:', buildDir);
    process.exit(1);
  }

  const missing = expectedEntries.filter((entry) => !files.includes(entry));
  if (missing.length > 0) {
    console.error('Missing expected entry files:', missing.join(', '));
    process.exit(1);
  }

  console.log('Build check passed.');
  console.log('Entry files:', expectedEntries.join(', '));

  const chunks = join(buildDir, 'chunks');
  try {
    const chunkFiles = readdirSync(chunks);
    const missingChunks = expectedChunks.filter(
      (entry) => !chunkFiles.includes(entry),
    );
    if (missingChunks.length > 0) {
      console.error('Missing expected chunk files:', missingChunks.join(', '));
      process.exit(1);
    }
    console.log('Chunks:', chunkFiles.join(', '));
  } catch {
    console.error('Build output chunks directory not found:', chunks);
    process.exit(1);
  }

  const totalSize = calculateDirSize(buildDir);
  console.log('Total output size:', formatBytes(totalSize));
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

checkBuild();

import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

interface FileAsset {
  url: string;
  path: string;
}

interface Config {
  files: FileAsset[];
}

async function computeSri(url: string): Promise<string> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to fetch ${url}: ${response.status}`);
  }
  const buffer = await response.arrayBuffer();
  const hash = createHash('sha384')
    .update(Buffer.from(buffer))
    .digest('base64');
  return `sha384-${hash}`;
}

async function main() {
  const configPath = resolve(import.meta.dirname, '../../assets/config.json');
  const outputPath = resolve(import.meta.dirname, '../../assets/sri.json');

  const config: Config = JSON.parse(readFileSync(configPath, 'utf-8'));
  const sri: Record<string, string> = {};

  for (const file of config.files) {
    process.stdout.write(`${file.path}...`);
    const integrity = await computeSri(file.url);
    sri[file.path] = integrity;
    process.stdout.write(` ${integrity.slice(0, 20)}...\n`);
  }

  writeFileSync(outputPath, JSON.stringify(sri, null, 2) + '\n');
  console.log(`\nWrote ${outputPath}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});

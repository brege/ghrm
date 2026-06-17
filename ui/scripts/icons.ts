import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import type { ReactElement } from 'react';
import { transformWithEsbuild } from 'vite';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const repoRoot = resolve(__dirname, '../..');
const sourcePath = join(__dirname, '../icons.tsx');
const compiledSourcePath = join(repoRoot, 'ui/.asset-check/icons-source.mjs');
const runtimeSpritePath = join(repoRoot, 'assets/js/icons.svg');
const obsoleteSourcePaths = [
  join(repoRoot, 'assets/templates/fragments/icons.html'),
  join(repoRoot, 'ui/icons.json'),
];
const refRoots = ['assets/templates', 'assets/css', 'src', 'ui/src'];
const productionRoots = ['ui/src'];
const skippedDirs = new Set(['.asset-check', '.vite-check', 'node_modules']);
const expectedIconCount = 60;
const spriteOpen = '<svg xmlns="http://www.w3.org/2000/svg">';
const spriteClose = '</svg>';

interface DynamicAskamaContract {
  expression: string;
  template: string;
  provider: string;
}

const dynamicAskamaContracts: DynamicAskamaContract[] = [
  {
    expression: 'row.icon',
    template: 'assets/templates/macros/stat.html',
    provider: 'src/http/about.rs',
  },
  {
    expression: 'name_header.icon',
    template: 'assets/templates/explorer.html',
    provider: 'src/explorer.rs',
  },
  {
    expression: 'column.icon',
    template: 'assets/templates/explorer.html',
    provider: 'src/explorer.rs',
  },
];

interface IconEntry {
  id: string;
  icon: ReactElement;
}

interface IconSource {
  icons: IconEntry[];
}

interface ParsedIcon {
  id: string;
  symbol: string;
}

interface IconData {
  icons: ParsedIcon[];
}

interface DynamicRef {
  expression: string;
  file: string;
}

function fail(message: string, ...parts: unknown[]): never {
  console.error(message, ...parts);
  process.exit(1);
}

function toPosix(path: string): string {
  return path.split(sep).join('/');
}

function readText(path: string): string {
  try {
    return readFileSync(path, 'utf8');
  } catch (error) {
    fail('Unable to read file:', path, (error as Error).message);
  }
}

function writeText(path: string, text: string): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text);
}

function checkSourceText(text: string): void {
  // Reject copied SVG payload in the reviewed icon declaration source.
  const svgPayloadPattern = /<\s*(?:path|svg|symbol)\b|d="/i;
  if (svgPayloadPattern.test(text)) {
    fail('Icon declaration source must not contain copied SVG payload.');
  }
}

async function loadSourceModule(): Promise<IconSource> {
  const sourceText = readText(sourcePath);
  checkSourceText(sourceText);

  const result = await transformWithEsbuild(sourceText, sourcePath, {
    format: 'esm',
    jsx: 'automatic',
    jsxImportSource: 'react',
    loader: 'tsx',
    target: 'es2022',
  });

  writeText(compiledSourcePath, result.code);

  try {
    return (await import(
      `${pathToFileURL(compiledSourcePath).href}?t=${Date.now()}`
    )) as IconSource;
  } catch (error) {
    fail('Unable to load icon declaration source:', (error as Error).message);
  }
}

interface ReactDOMServer {
  renderToStaticMarkup: (element: ReactElement) => string;
}

async function loadRenderer(): Promise<ReactDOMServer> {
  try {
    return (await import('react-dom/server')) as ReactDOMServer;
  } catch (error) {
    fail('Unable to load React static renderer:', (error as Error).message);
  }
}

function parseAttrs(rawAttrs: string): Map<string, string> {
  const attrs = new Map<string, string>();

  // Matches XML attribute assignments in the rendered SVG opening tag.
  const attrPattern = /\s([A-Za-z_:][A-Za-z0-9_.:-]*)="([^"]*)"/g;
  for (const match of rawAttrs.matchAll(attrPattern)) {
    if (attrs.has(match[1])) {
      fail('Rendered icon has duplicate SVG attribute:', match[1]);
    }
    attrs.set(match[1], match[2]);
  }

  return attrs;
}

function formatAttrs(attrs: [string, string][]): string {
  return attrs.map(([name, value]) => `${name}="${value}"`).join(' ');
}

function renderSymbol(id: string, rendered: string): string {
  const trimmed = rendered.trim();

  // Captures the rendered SVG opening attributes and child markup.
  const svgPattern = /^<svg\b([^>]*)>([\s\S]*)<\/svg>$/;
  const match = trimmed.match(svgPattern);
  if (!match) {
    fail('Rendered icon did not produce a single SVG element:', id);
  }

  const attrs = parseAttrs(match[1]);
  const viewBox = attrs.get('viewBox');
  if (!viewBox) {
    fail('Rendered icon is missing viewBox:', id);
  }

  const ignoredAttrs = new Set(['height', 'role', 'width', 'xmlns']);
  const symbolAttrs: [string, string][] = [];
  for (const [name, value] of attrs) {
    if (ignoredAttrs.has(name) || name === 'viewBox') continue;
    symbolAttrs.push([name, value]);
  }
  symbolAttrs.sort(([left], [right]) => left.localeCompare(right));
  symbolAttrs.unshift(['id', id], ['viewBox', viewBox]);

  return `    <symbol ${formatAttrs(symbolAttrs)}>${match[2]}</symbol>`;
}

async function readSource(): Promise<IconData> {
  const source = await loadSourceModule();
  const { renderToStaticMarkup } = await loadRenderer();

  if (!Array.isArray(source.icons)) {
    fail('Icon source must export an icons array.');
  }
  if (source.icons.length !== expectedIconCount) {
    fail(
      'Icon source has unexpected icon count:',
      `${source.icons.length}, expected ${expectedIconCount}`,
    );
  }

  const ids = new Set<string>();
  const icons: ParsedIcon[] = [];
  for (const icon of source.icons) {
    if (typeof icon.id !== 'string' || !icon.id.startsWith('ghrm-icon-')) {
      fail('Invalid icon id:', String(icon.id));
    }
    if (ids.has(icon.id)) {
      fail('Duplicate icon id:', icon.id);
    }
    if (typeof icon.icon !== 'object' || icon.icon === null) {
      fail('Icon entry is missing an imported component tag:', icon.id);
    }

    ids.add(icon.id);
    icons.push({
      id: icon.id,
      symbol: renderSymbol(icon.id, renderToStaticMarkup(icon.icon)),
    });
  }

  return { icons };
}

function renderSprite(data: IconData): string {
  return `${spriteOpen}\n${data.icons.map((icon) => icon.symbol).join('\n')}\n${spriteClose}\n`;
}

export async function generateSprite(): Promise<string> {
  return renderSprite(await readSource());
}

function checkSpriteShape(data: IconData): void {
  const sprite = renderSprite(data);
  if (!sprite.startsWith(`${spriteOpen}\n`) || !sprite.endsWith('\n</svg>\n')) {
    fail('Generated icon sprite has an unexpected SVG wrapper.');
  }

  const expectedIds = sourceIds(data);
  const symbolIds: string[] = [];

  // Matches generated symbol opening tags and extracts their normalized attrs.
  const symbolPattern = /<symbol\b([^>]*)>/g;
  for (const match of sprite.matchAll(symbolPattern)) {
    const attrs = parseAttrs(match[1]);
    const id = attrs.get('id');
    if (!id) {
      fail('Generated icon sprite contains a symbol without an id.');
    }
    if (!attrs.has('viewBox')) {
      fail('Generated icon sprite contains a symbol without viewBox:', id);
    }
    symbolIds.push(id);
  }

  if (symbolIds.length !== data.icons.length) {
    fail(
      'Generated icon sprite has unexpected symbol count:',
      `${symbolIds.length}, expected ${data.icons.length}`,
    );
  }

  const duplicates = symbolIds.filter(
    (id, index) => symbolIds.indexOf(id) !== index,
  );
  if (duplicates.length > 0) {
    fail(
      'Generated icon sprite contains duplicate symbols:',
      duplicates.join(', '),
    );
  }

  const missing = [...expectedIds].filter((id) => !symbolIds.includes(id));
  if (missing.length > 0) {
    fail('Generated icon sprite is missing source IDs:', missing.join(', '));
  }
}

function walkFiles(dir: string): string[] {
  const files: string[] = [];
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

function collectIconRefs(): {
  refs: Map<string, Set<string>>;
  dynamicRefs: DynamicRef[];
} {
  const refs = new Map<string, Set<string>>();
  const dynamicRefs: DynamicRef[] = [];

  // Matches literal ghrm sprite IDs in Rust, templates, CSS, and TS.
  const iconRefPattern = /ghrm-icon-[A-Za-z0-9-]+/g;

  // Matches TS helper calls that expand to ghrm-icon-* at runtime.
  const tsIconCallPattern = /\bicon\('([A-Za-z0-9-]+)'/g;

  // Matches Rust alert icon branches that expand to ghrm-icon-* at runtime.
  const rustAlertPattern =
    /^\s*"([A-Za-z0-9-]+)"\s*=>\s*Some\("([A-Za-z0-9-]+)"\),/gm;

  // Matches Askama icon expressions whose concrete IDs are supplied by Rust.
  const askamaIconPattern = /href="[^"]*#\{\{\s*([A-Za-z0-9_.]+)\s*\}\}"/g;

  function addRef(id: string, file: string, kind: string): void {
    if (!refs.has(id)) refs.set(id, new Set());
    refs.get(id)!.add(`${toPosix(relative(repoRoot, file))} [${kind}]`);
  }

  for (const root of refRoots) {
    for (const file of walkFiles(join(repoRoot, root))) {
      if (file === sourcePath) continue;

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

function sourceIds(data: IconData): Set<string> {
  return new Set(data.icons.map((icon) => icon.id));
}

function refsByProvider(
  refs: Map<string, Set<string>>,
  provider: string,
): string[] {
  const matches: string[] = [];
  for (const [id, files] of refs) {
    for (const file of files) {
      if (file.startsWith(`${provider} `)) {
        matches.push(id);
      }
    }
  }
  return [...new Set(matches)].sort();
}

function checkDynamicRefs(
  refs: Map<string, Set<string>>,
  dynamicRefs: DynamicRef[],
): void {
  const missing: string[] = [];

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

function checkRefs(
  data: IconData,
  refs: Map<string, Set<string>>,
  dynamicRefs: DynamicRef[],
): void {
  const iconIds = sourceIds(data);
  const refIds = new Set(refs.keys());
  const missing: string[] = [];
  const unreferenced: string[] = [];

  for (const id of refIds) {
    if (!iconIds.has(id)) {
      missing.push(
        `${id} referenced by ${[...refs.get(id)!].sort().join(', ')}`,
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

function reportRefs(
  refs: Map<string, Set<string>>,
  dynamicRefs: DynamicRef[],
): void {
  console.log('Icon reference report:');
  for (const id of [...refs.keys()].sort()) {
    console.log(`- ${id}: ${[...refs.get(id)!].sort().join(', ')}`);
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

function writeGenerated(data: IconData): void {
  writeText(runtimeSpritePath, renderSprite(data));
}

function checkNoObsoleteSources(): void {
  const obsolete = obsoleteSourcePaths
    .filter((path) => existsSync(path))
    .map((path) => toPosix(relative(repoRoot, path)));
  if (obsolete.length > 0) {
    fail('Obsolete icon source artifacts remain:', obsolete.join(', '));
  }
}

function checkProductionImports(): void {
  const offenders: string[] = [];

  // Matches static and dynamic imports of the icon source packages.
  const forbiddenImportPattern =
    /\bfrom\s+['"](?:react|react-dom(?:\/[^'"]*)?|react-icons(?:\/[^'"]*)?)['"]|\bimport\s*\(\s*['"](?:react|react-dom(?:\/[^'"]*)?|react-icons(?:\/[^'"]*)?)['"]\s*\)/;

  for (const root of productionRoots) {
    for (const file of walkFiles(join(repoRoot, root))) {
      if (forbiddenImportPattern.test(readText(file))) {
        offenders.push(toPosix(relative(repoRoot, file)));
      }
    }
  }

  if (offenders.length > 0) {
    fail(
      'Production UI source imports icon-generation packages:',
      offenders.join(', '),
    );
  }
}

async function main(): Promise<void> {
  const mode = process.argv[2] ?? 'check';
  if (mode === 'write') {
    writeGenerated(await readSource());
  } else if (mode === 'check') {
    const data = await readSource();
    checkNoObsoleteSources();
    checkSpriteShape(data);
    checkProductionImports();
    const { refs, dynamicRefs } = collectIconRefs();
    checkRefs(data, refs, dynamicRefs);
    reportRefs(refs, dynamicRefs);
    console.log('Icon sprite check passed.');
  } else {
    fail('Unknown icon mode:', mode);
  }
}

if (resolve(process.argv[1] ?? '') === __filename) {
  main().catch((error) => {
    fail('Icon command failed:', (error as Error).message);
  });
}

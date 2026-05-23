import { renderBlobs } from './adapters/code';
import { showCopied, writeClipboard } from './adapters/copy';
import { populateDates } from './explorer';
import { applyWrapState, getWrapPref, setWrapPref } from './prefs';

interface GistArticle extends HTMLElement {
  dataset: DOMStringMap & {
    ghrmGistId?: string;
    ghrmGistPage?: string;
    ghrmGistReady?: string;
  };
}

interface GistStashArticle extends HTMLElement {
  dataset: DOMStringMap & {
    ghrmGistReady?: string;
  };
}

interface GistRow extends HTMLElement {
  dataset: DOMStringMap & {
    ghrmGistId: string;
  };
}

interface GistNameInput extends HTMLInputElement {
  dataset: DOMStringMap & {
    ghrmGistSaved?: string;
  };
}

interface GistTextarea extends HTMLTextAreaElement {
  dataset: DOMStringMap & {
    ghrmGistSaved?: string;
  };
}

interface GistCopyButton extends HTMLButtonElement {
  _ghrmCopyReset?: ReturnType<typeof setTimeout> | null;
}

interface GistRowInput extends HTMLInputElement {
  dataset: DOMStringMap & {
    ghrmGistRowInput?: string;
    ghrmSaving?: string;
  };
}

interface LineRange {
  lineStart: number;
  lineEnd: number;
}

interface Removal {
  position: number;
  size: number;
}

interface UndentBlock {
  text: string;
  removals: Removal[];
}

interface IndentEdit {
  start: number;
  end: number;
  text: string;
  selectionStart: number;
  selectionEnd: number;
}

interface RenameResponse {
  id: string;
  href: string;
  name: string;
}

const gistPath = '/_ghrm/gist';
const stashPath = '/_ghrm/gist/stash';
const indentText = '  ';
const nameMax = 80;

let liveBound = false;
let resizeBound = false;
let pendingGistRefresh = false;

function currentArticle(): GistArticle | null {
  return document.querySelector<GistArticle>('article[data-ghrm-gist]');
}

function currentStash(): GistStashArticle | null {
  return document.querySelector<GistStashArticle>(
    'article[data-ghrm-gist-stash]',
  );
}

function currentGistPath(article: GistArticle | null): string {
  return article?.dataset.ghrmGistPage || gistPath;
}

function currentText(article: GistArticle): string {
  return (
    article.querySelector<GistTextarea>('[data-ghrm-gist-form] textarea')
      ?.value || ''
  );
}

function hasUnsavedChanges(article: GistArticle | null): boolean {
  const input = article?.querySelector<GistTextarea>(
    '[data-ghrm-gist-form] textarea',
  );
  if (!input) return false;
  const name = nameInput(article);
  const normalized = name ? normalizeName(name.value) : '';
  return (
    input.value !== input.dataset.ghrmGistSaved ||
    (name && normalized !== name.dataset.ghrmGistSaved)
  );
}

function refreshPendingGist(article: GistArticle): void {
  if (!pendingGistRefresh || hasUnsavedChanges(article)) return;
  refreshGist();
}

function requestGistRefresh(article: GistArticle): void {
  if (hasUnsavedChanges(article)) {
    pendingGistRefresh = true;
    return;
  }
  refreshGist();
}

function pad(value: number, width: number): string {
  return String(value).padStart(width, '0');
}

function defaultGistName(): string {
  const now = new Date();
  return `${now.getUTCFullYear()}${pad(now.getUTCMonth() + 1, 2)}${pad(now.getUTCDate(), 2)}T${pad(now.getUTCHours(), 2)}${pad(now.getUTCMinutes(), 2)}${pad(now.getUTCSeconds(), 2)}.${pad(now.getUTCMilliseconds(), 3)}000000Z`;
}

function normalizeName(value: string): string {
  const name = value.trim();
  return name.endsWith('.txt') ? name.slice(0, -4) : name;
}

function validName(name: string): boolean {
  return (
    name.length <= nameMax &&
    (name === '' ||
      (name !== '.' &&
        name !== '..' &&
        !name.startsWith('.') &&
        !name.endsWith('.') &&
        /^[A-Za-z0-9._-]+$/.test(name)))
  );
}

function nameInput(article: GistArticle): GistNameInput | null {
  return article.querySelector<GistNameInput>('[data-ghrm-gist-name]');
}

function syncSaveAction(article: GistArticle, saving = false): void {
  const input = article.querySelector<GistTextarea>(
    '[data-ghrm-gist-form] textarea',
  );
  const name = nameInput(article);
  const control = article.querySelector<HTMLElement>(
    '[data-ghrm-gist-save-control]',
  );
  const button = article.querySelector<HTMLButtonElement>(
    '[data-ghrm-gist-save]',
  );
  if (!input || !button) return;

  const normalized = name ? normalizeName(name.value) : '';
  const valid = !name || validName(normalized);
  const changed =
    input.value !== input.dataset.ghrmGistSaved ||
    (name && normalized !== name.dataset.ghrmGistSaved);
  button.disabled = saving || !valid || !changed;
  const label = saving
    ? 'Saving'
    : !valid
      ? 'Use letters, numbers, dots, dashes, or underscores'
      : changed
        ? 'Save paste'
        : 'No changes to save';
  button.setAttribute('aria-label', label);
  button.title = label;
  name?.setAttribute('aria-invalid', valid ? 'false' : 'true');
  if (control) {
    control.title = label;
  }
}

function syncEditor(article: GistArticle): void {
  const editor = article.querySelector<HTMLElement>('[data-ghrm-gist-editor]');
  const input = article.querySelector<GistTextarea>(
    '[data-ghrm-gist-form] textarea',
  );
  const blob = article.querySelector<HTMLElement>('.ghrm-blob');
  if (!editor || !input || !blob) return;

  input.style.height = 'auto';
  const height = Math.max(
    input.scrollHeight,
    blob.offsetHeight,
    editor.clientHeight,
  );
  input.style.height = `${height}px`;
  blob.scrollLeft = input.scrollLeft;
}

function syncEditorSoon(article: GistArticle): void {
  requestAnimationFrame(() => {
    syncEditor(article);
  });
}

function syncBlob(article: GistArticle): void {
  const input = article.querySelector<GistTextarea>(
    '[data-ghrm-gist-form] textarea',
  );
  const source = article.querySelector<HTMLElement>('.ghrm-blob-source code');
  const data = article.querySelector<HTMLTemplateElement>('template.ghrm-data');
  if (!input || !source) return;

  const text = input.value;
  if (source.textContent !== text) {
    source.textContent = text;
    delete source.dataset.ghrmHighlighted;
  }
  if (data?.content) {
    data.content.textContent = text;
  }

  renderBlobs();
  syncSaveAction(article);
  syncEditorSoon(article);
}

function syncBlobScroll(article: GistArticle): void {
  const input = article.querySelector<GistTextarea>(
    '[data-ghrm-gist-form] textarea',
  );
  const blob = article.querySelector<HTMLElement>('.ghrm-blob');
  if (!input || !blob) return;

  blob.scrollLeft = input.scrollLeft;
}

// Indent edits operate on whole lines, while selection offsets follow the original caret range.
function selectedLineRange(
  text: string,
  start: number,
  end: number,
): LineRange {
  const lineStart = start === 0 ? 0 : text.lastIndexOf('\n', start - 1) + 1;
  const endRef = end > start && text[end - 1] === '\n' ? end - 1 : end;
  const nextBreak = text.indexOf('\n', endRef);
  const lineEnd = nextBreak === -1 ? text.length : nextBreak;
  return { lineStart, lineEnd };
}

function lineStarts(text: string, start: number, end: number): number[] {
  const starts = [start];
  for (let i = start; i < end; i += 1) {
    if (text[i] === '\n' && i + 1 < end) {
      starts.push(i + 1);
    }
  }
  return starts;
}

function shiftAfterInsert(offset: number, positions: number[]): number {
  return (
    offset +
    positions.filter((position) => position < offset).length * indentText.length
  );
}

function linePrefixLen(line: string): number {
  if (line.startsWith(indentText)) return indentText.length;
  if (line.startsWith('\t') || line.startsWith(' ')) return 1;
  return 0;
}

function shiftAfterRemoval(offset: number, removals: Removal[]): number {
  let next = offset;
  for (const removal of removals) {
    if (offset >= removal.position + removal.size) {
      next -= removal.size;
    } else if (offset > removal.position) {
      next -= offset - removal.position;
    }
  }
  return next;
}

function undentBlock(text: string, start: number, end: number): UndentBlock {
  const lines = text.slice(start, end).split('\n');
  const removals: Removal[] = [];
  const out: string[] = [];
  let position = start;

  for (const line of lines) {
    const size = linePrefixLen(line);
    if (size > 0) {
      removals.push({ position, size });
    }
    out.push(line.slice(size));
    position += line.length + 1;
  }

  return { text: out.join('\n'), removals };
}

function indentEdit(
  text: string,
  start: number,
  end: number,
  outdent: boolean,
): IndentEdit {
  if (!outdent && start === end) {
    return {
      start,
      end,
      text: indentText,
      selectionStart: start + indentText.length,
      selectionEnd: start + indentText.length,
    };
  }

  const { lineStart, lineEnd } = selectedLineRange(text, start, end);
  if (outdent) {
    const block = undentBlock(text, lineStart, lineEnd);
    return {
      start: lineStart,
      end: lineEnd,
      text: block.text,
      selectionStart: shiftAfterRemoval(start, block.removals),
      selectionEnd: shiftAfterRemoval(end, block.removals),
    };
  }

  const starts = lineStarts(text, lineStart, lineEnd);
  return {
    start: lineStart,
    end: lineEnd,
    text: text
      .slice(lineStart, lineEnd)
      .split('\n')
      .map((line) => `${indentText}${line}`)
      .join('\n'),
    selectionStart: shiftAfterInsert(start, starts),
    selectionEnd: shiftAfterInsert(end, starts),
  };
}

function handleIndentKey(event: KeyboardEvent, article: GistArticle): void {
  if (
    event.key !== 'Tab' ||
    event.altKey ||
    event.ctrlKey ||
    event.metaKey ||
    event.isComposing
  ) {
    return;
  }

  event.preventDefault();
  const input = event.currentTarget as GistTextarea;
  const edit = indentEdit(
    input.value,
    input.selectionStart,
    input.selectionEnd,
    event.shiftKey,
  );
  input.setRangeText(edit.text, edit.start, edit.end, 'preserve');
  input.setSelectionRange(edit.selectionStart, edit.selectionEnd);
  syncBlob(article);
}

function setStatus(article: GistArticle | null, message: string): void {
  const status = article?.querySelector<HTMLElement>('[data-ghrm-gist-status]');
  if (status) {
    status.textContent = message;
  }
}

function replaceGistUrl(): void {
  if (window.location.pathname !== gistPath) {
    window.history.replaceState(window.history.state, '', gistPath);
  }
}

async function save(article: GistArticle): Promise<void> {
  const input = article.querySelector<GistTextarea>(
    '[data-ghrm-gist-form] textarea',
  );
  if (!input) return;
  const name = nameInput(article);
  const normalized = name ? normalizeName(name.value) : '';
  if (name && !validName(normalized)) {
    syncSaveAction(article);
    return;
  }
  if (
    input.value === input.dataset.ghrmGistSaved &&
    (!name || normalized === name.dataset.ghrmGistSaved)
  ) {
    syncSaveAction(article);
    return;
  }
  syncSaveAction(article, true);
  setStatus(article, 'Saving');
  const headers: Record<string, string> = {
    Accept: 'application/json',
    'Content-Type': 'text/plain; charset=utf-8',
  };
  if (normalized) {
    headers['X-Ghrm-Gist-Name'] = normalized;
  }
  if (article.dataset.ghrmGistId) {
    headers['X-Ghrm-Gist-Id'] = article.dataset.ghrmGistId;
  }
  try {
    const response = await fetch(gistPath, {
      method: 'POST',
      headers,
      body: input.value,
    });
    if (!response.ok) {
      throw new Error(`gist save failed: ${response.status}`);
    }
    const next = await refreshGist(gistPath);
    if (next) {
      replaceGistUrl();
      setStatus(next, 'Saved');
    } else {
      syncSaveAction(article);
    }
  } catch {
    setStatus(article, 'Save failed');
    syncSaveAction(article);
  }
}

async function refreshArticle<T extends GistArticle | GistStashArticle>(
  article: T | null,
  path: string,
  selector: string,
): Promise<T | null> {
  if (!article) return null;
  const response = await fetch(path, {
    headers: {
      Accept: 'text/html',
      'HX-Request': 'true',
    },
  });
  if (!response.ok) {
    setStatus(article, 'Refresh failed');
    return null;
  }

  const html = await response.text();
  const doc = new DOMParser().parseFromString(html, 'text/html');
  const next = doc.querySelector<T>(selector);
  if (!next) {
    setStatus(article, 'Refresh failed');
    return null;
  }

  article.replaceWith(next);
  populateDates();
  document.dispatchEvent(new CustomEvent('ghrm:contentready'));
  return next;
}

async function refreshGist(
  path = currentGistPath(currentArticle()),
): Promise<GistArticle | null> {
  const next = await refreshArticle(
    currentArticle(),
    path,
    'article[data-ghrm-gist]',
  );
  if (next) {
    pendingGistRefresh = false;
  }
  setupGist();
  return next;
}

async function refreshStash(): Promise<GistStashArticle | null> {
  return refreshArticle(
    currentStash(),
    stashPath,
    'article[data-ghrm-gist-stash]',
  );
}

function syncWrapToggle(article: GistArticle): void {
  const toggle = article.querySelector<HTMLElement>('[data-ghrm-gist-wrap]');
  const input = article.querySelector<GistTextarea>(
    '[data-ghrm-gist-form] textarea',
  );
  if (!toggle || !input) return;

  const wrap = getWrapPref();
  toggle.classList.toggle('is-active', wrap);
  toggle.setAttribute('aria-pressed', wrap ? 'true' : 'false');
  const label = wrap ? 'Disable line wrap' : 'Wrap lines';
  toggle.setAttribute('aria-label', label);
  toggle.title = label;
  input.setAttribute('wrap', wrap ? 'soft' : 'off');
  applyWrapState(wrap);
  syncEditorSoon(article);
}

function setupGistEditor(article: GistArticle): void {
  if (article.dataset.ghrmGistReady === '1') return;
  article.dataset.ghrmGistReady = '1';

  const form = article.querySelector<HTMLFormElement>('[data-ghrm-gist-form]');
  form?.addEventListener('submit', (event) => {
    event.preventDefault();
    save(article);
  });

  const saveButton = article.querySelector<HTMLButtonElement>(
    '[data-ghrm-gist-save]',
  );
  saveButton?.addEventListener('click', () => {
    save(article);
  });

  const input = article.querySelector<GistTextarea>(
    '[data-ghrm-gist-form] textarea',
  );
  if (input) {
    input.dataset.ghrmGistSaved = input.value;
  }
  const name = nameInput(article);
  if (name) {
    if (!name.value) {
      name.value = defaultGistName();
    }
    name.dataset.ghrmGistSaved =
      article.dataset.ghrmGistId || normalizeName(name.value);
    name.addEventListener('input', () => {
      syncSaveAction(article);
      refreshPendingGist(article);
    });
  }
  input?.addEventListener('input', () => {
    syncBlob(article);
    refreshPendingGist(article);
  });
  input?.addEventListener('keydown', (event) => {
    handleIndentKey(event, article);
  });
  input?.addEventListener('scroll', () => {
    syncBlobScroll(article);
  });

  const copy = article.querySelector<GistCopyButton>('[data-ghrm-gist-copy]');
  copy?.addEventListener('click', async () => {
    await writeClipboard(currentText(article));
    showCopied(copy);
  });

  const wrap = article.querySelector<HTMLElement>('[data-ghrm-gist-wrap]');
  wrap?.addEventListener('click', () => {
    setWrapPref(!getWrapPref());
    syncWrapToggle(article);
  });
  syncWrapToggle(article);
  syncSaveAction(article);
  renderBlobs();
  syncEditorSoon(article);
}

function rowRenameUrl(row: GistRow): string {
  return `/_ghrm/gist/rename/${encodeURIComponent(row.dataset.ghrmGistId)}`;
}

function restoreRowRename(cell: Element, input: GistRowInput): void {
  const link = cell.querySelector<HTMLElement>('[data-ghrm-gist-row-link]');
  const button = cell.querySelector<HTMLElement>(
    '[data-ghrm-gist-rename-start]',
  );
  input.remove();
  if (link) {
    link.hidden = false;
  }
  if (button) {
    button.hidden = false;
  }
}

function renameResponse(value: unknown): RenameResponse {
  if (
    typeof value !== 'object' ||
    value === null ||
    typeof (value as RenameResponse).id !== 'string' ||
    typeof (value as RenameResponse).href !== 'string' ||
    typeof (value as RenameResponse).name !== 'string'
  ) {
    throw new Error('invalid gist rename response');
  }
  return value as RenameResponse;
}

async function saveRowRename(
  row: GistRow,
  cell: Element,
  input: GistRowInput,
): Promise<void> {
  if (input.dataset.ghrmSaving === '1') return;
  const link = cell.querySelector<HTMLAnchorElement>(
    '[data-ghrm-gist-row-link]',
  );
  const next = normalizeName(input.value);
  const current = normalizeName(link?.textContent || '');
  if (!validName(next)) {
    input.setAttribute('aria-invalid', 'true');
    input.title = 'Use letters, numbers, dots, dashes, or underscores';
    input.focus();
    return;
  }
  if (next === current) {
    restoreRowRename(cell, input);
    return;
  }

  input.dataset.ghrmSaving = '1';
  const response = await fetch(rowRenameUrl(row), {
    method: 'POST',
    headers: {
      Accept: 'application/json',
      'Content-Type': 'text/plain; charset=utf-8',
    },
    body: next,
  });
  if (!response.ok) {
    input.dataset.ghrmSaving = '0';
    input.setAttribute('aria-invalid', 'true');
    input.title = 'Name already exists or is invalid';
    input.focus();
    return;
  }

  const renamed = renameResponse(await response.json());
  row.dataset.ghrmGistId = renamed.id;
  if (link) {
    link.href = renamed.href;
    link.textContent = renamed.name;
  }
  restoreRowRename(cell, input);
}

function beginRowRename(row: GistRow): void {
  const cell = row.querySelector('.ghrm-gist-name-cell');
  const link = cell?.querySelector<HTMLAnchorElement>(
    '[data-ghrm-gist-row-link]',
  );
  const button = cell?.querySelector<HTMLElement>(
    '[data-ghrm-gist-rename-start]',
  );
  if (!cell || !link || cell.querySelector('[data-ghrm-gist-row-input]'))
    return;

  const input = document.createElement('input') as GistRowInput;
  input.type = 'text';
  input.className = 'ghrm-gist-row-input';
  input.dataset.ghrmGistRowInput = '1';
  input.value = link.textContent || '';
  input.setAttribute('aria-label', 'Paste filename');
  input.autocomplete = 'off';
  input.spellcheck = false;

  link.hidden = true;
  if (button) {
    button.hidden = true;
  }
  cell.insertBefore(input, link);
  input.focus();
  input.select();

  input.addEventListener('keydown', (event) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      saveRowRename(row, cell, input);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      restoreRowRename(cell, input);
    }
  });
  input.addEventListener('blur', () => {
    if (input.isConnected) {
      saveRowRename(row, cell, input);
    }
  });
}

function setupGistStash(stash: GistStashArticle): void {
  if (stash.dataset.ghrmGistReady === '1') return;
  stash.dataset.ghrmGistReady = '1';
  for (const row of stash.querySelectorAll<GistRow>('[data-ghrm-gist-row]')) {
    const button = row.querySelector<HTMLElement>(
      '[data-ghrm-gist-rename-start]',
    );
    button?.addEventListener('click', () => {
      beginRowRename(row);
    });
  }
}

export function setupGist(): void {
  const article = currentArticle();
  if (article) {
    setupGistEditor(article);
  }

  const stash = currentStash();
  if (stash) {
    setupGistStash(stash);
  }
}

function setupLiveGist(): void {
  if (liveBound) return;
  liveBound = true;
  document.addEventListener('ghrm:live:gist', () => {
    const article = currentArticle();
    if (article) {
      requestGistRefresh(article);
    } else if (currentStash()) {
      refreshStash();
    }
  });
}

function setupResizeGist(): void {
  if (resizeBound) return;
  resizeBound = true;
  window.addEventListener('resize', () => {
    const article = currentArticle();
    if (article) {
      syncEditorSoon(article);
    }
  });
}

document.addEventListener('DOMContentLoaded', () => {
  setupLiveGist();
  setupResizeGist();
  setupGist();
});

document.addEventListener('ghrm:contentready', setupGist);

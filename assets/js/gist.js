import { renderBlobs } from './adapters/code.js';
import { showCopied, writeClipboard } from './adapters/copy.js';
import { applyWrapState, getWrapPref, setWrapPref } from './prefs.js';

const gistPath = '/_ghrm/gist';
const stashPath = '/_ghrm/gist/stash';
const indentText = '  ';

let liveBound = false;
let resizeBound = false;

function currentArticle() {
  return document.querySelector('article[data-ghrm-gist]');
}

function currentStash() {
  return document.querySelector('article[data-ghrm-gist-stash]');
}

function currentGistPath(article) {
  return article?.dataset.ghrmGistPage || gistPath;
}

function currentText(article) {
  return article?.querySelector('[data-ghrm-gist-form] textarea')?.value || '';
}

function syncSaveAction(article, saving = false) {
  const input = article.querySelector('[data-ghrm-gist-form] textarea');
  const control = article.querySelector('[data-ghrm-gist-save-control]');
  const button = article.querySelector('[data-ghrm-gist-save]');
  if (!input || !button) return;

  const changed = input.value !== input.dataset.ghrmGistSaved;
  button.disabled = saving || !changed;
  const label = saving
    ? 'Saving'
    : changed
      ? 'Save paste'
      : 'No changes to save';
  button.setAttribute('aria-label', label);
  button.title = label;
  if (control) {
    control.title = label;
  }
}

function syncEditor(article) {
  const editor = article.querySelector('[data-ghrm-gist-editor]');
  const input = article.querySelector('[data-ghrm-gist-form] textarea');
  const blob = article.querySelector('.ghrm-blob');
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

function syncEditorSoon(article) {
  requestAnimationFrame(() => {
    syncEditor(article);
  });
}

function syncBlob(article) {
  const input = article.querySelector('[data-ghrm-gist-form] textarea');
  const source = article.querySelector('.ghrm-blob-source code');
  const data = article.querySelector('template.ghrm-data');
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

function syncBlobScroll(article) {
  const input = article.querySelector('[data-ghrm-gist-form] textarea');
  const blob = article.querySelector('.ghrm-blob');
  if (!input || !blob) return;

  blob.scrollLeft = input.scrollLeft;
}

// Indent edits operate on whole lines, while selection offsets follow the original caret range.
function selectedLineRange(text, start, end) {
  const lineStart = start === 0 ? 0 : text.lastIndexOf('\n', start - 1) + 1;
  const endRef = end > start && text[end - 1] === '\n' ? end - 1 : end;
  const nextBreak = text.indexOf('\n', endRef);
  const lineEnd = nextBreak === -1 ? text.length : nextBreak;
  return { lineStart, lineEnd };
}

function lineStarts(text, start, end) {
  const starts = [start];
  for (let i = start; i < end; i += 1) {
    if (text[i] === '\n' && i + 1 < end) {
      starts.push(i + 1);
    }
  }
  return starts;
}

function shiftAfterInsert(offset, positions) {
  return (
    offset +
    positions.filter((position) => position < offset).length * indentText.length
  );
}

function linePrefixLen(line) {
  if (line.startsWith(indentText)) return indentText.length;
  if (line.startsWith('\t') || line.startsWith(' ')) return 1;
  return 0;
}

function shiftAfterRemoval(offset, removals) {
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

function undentBlock(text, start, end) {
  const lines = text.slice(start, end).split('\n');
  const removals = [];
  const out = [];
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

function indentEdit(text, start, end, outdent) {
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

function handleIndentKey(event, article) {
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
  const input = event.currentTarget;
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

function setStatus(article, message) {
  const status = article?.querySelector('[data-ghrm-gist-status]');
  if (status) {
    status.textContent = message;
  }
}

function replaceGistUrl() {
  if (window.location.pathname !== gistPath) {
    window.history.replaceState(window.history.state, '', gistPath);
  }
}

async function save(article) {
  const input = article.querySelector('[data-ghrm-gist-form] textarea');
  if (!input) return;
  if (input.value === input.dataset.ghrmGistSaved) {
    syncSaveAction(article);
    return;
  }
  syncSaveAction(article, true);
  setStatus(article, 'Saving');
  try {
    const response = await fetch(gistPath, {
      method: 'POST',
      headers: {
        Accept: 'application/json',
        'Content-Type': 'text/plain; charset=utf-8',
      },
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

async function refreshArticle(article, path, selector) {
  if (!article) return;
  const response = await fetch(path, {
    headers: {
      Accept: 'text/html',
      'HX-Request': 'true',
    },
  });
  if (!response.ok) {
    setStatus(article, 'Refresh failed');
    return;
  }

  const html = await response.text();
  const doc = new DOMParser().parseFromString(html, 'text/html');
  const next = doc.querySelector(selector);
  if (!next) {
    setStatus(article, 'Refresh failed');
    return;
  }

  article.replaceWith(next);
  document.dispatchEvent(new CustomEvent('ghrm:contentready'));
  return next;
}

async function refreshGist(path = currentGistPath(currentArticle())) {
  const next = await refreshArticle(
    currentArticle(),
    path,
    'article[data-ghrm-gist]',
  );
  setupGist();
  return next;
}

async function refreshStash() {
  return refreshArticle(
    currentStash(),
    stashPath,
    'article[data-ghrm-gist-stash]',
  );
}

function syncWrapToggle(article) {
  const toggle = article.querySelector('[data-ghrm-gist-wrap]');
  const input = article.querySelector('[data-ghrm-gist-form] textarea');
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

export function setupGist() {
  const article = currentArticle();
  if (!article || article.dataset.ghrmGistReady === '1') return;
  article.dataset.ghrmGistReady = '1';

  const form = article.querySelector('[data-ghrm-gist-form]');
  form?.addEventListener('submit', (event) => {
    event.preventDefault();
    save(article);
  });

  const saveButton = article.querySelector('[data-ghrm-gist-save]');
  saveButton?.addEventListener('click', () => {
    save(article);
  });

  const input = article.querySelector('[data-ghrm-gist-form] textarea');
  if (input) {
    input.dataset.ghrmGistSaved = input.value;
  }
  input?.addEventListener('input', () => {
    syncBlob(article);
  });
  input?.addEventListener('keydown', (event) => {
    handleIndentKey(event, article);
  });
  input?.addEventListener('scroll', () => {
    syncBlobScroll(article);
  });

  const copy = article.querySelector('[data-ghrm-gist-copy]');
  copy?.addEventListener('click', async () => {
    await writeClipboard(currentText(article));
    showCopied(copy);
  });

  const wrap = article.querySelector('[data-ghrm-gist-wrap]');
  wrap?.addEventListener('click', () => {
    setWrapPref(!getWrapPref());
    syncWrapToggle(article);
  });
  syncWrapToggle(article);
  syncSaveAction(article);
  renderBlobs();
  syncEditorSoon(article);
}

function setupLiveGist() {
  if (liveBound) return;
  liveBound = true;
  document.addEventListener('ghrm:live:gist', () => {
    if (currentArticle()) {
      refreshGist();
    } else if (currentStash()) {
      refreshStash();
    }
  });
}

function setupResizeGist() {
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

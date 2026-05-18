import { renderBlobs } from './adapters/code.js';
import { showCopied, writeClipboard } from './adapters/copy.js';
import { applyWrapState, getWrapPref, setWrapPref } from './prefs.js';

const gistPath = '/_ghrm/gist';
const stashPath = '/_ghrm/gist/stash';

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
  syncEditorSoon(article);
}

function syncBlobScroll(article) {
  const input = article.querySelector('[data-ghrm-gist-form] textarea');
  const blob = article.querySelector('.ghrm-blob');
  if (!input || !blob) return;

  blob.scrollLeft = input.scrollLeft;
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

async function publish(article) {
  const input = article.querySelector('[data-ghrm-gist-form] textarea');
  const button = article.querySelector('.ghrm-gist-submit');
  if (!input) return;
  if (button) {
    button.disabled = true;
  }
  setStatus(article, 'Publishing');
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
      throw new Error(`gist publish failed: ${response.status}`);
    }
    const next = await refreshGist(gistPath);
    if (next) {
      replaceGistUrl();
      setStatus(next, 'Published');
    }
  } catch {
    setStatus(article, 'Publish failed');
  } finally {
    if (button) {
      button.disabled = false;
    }
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
    publish(article);
  });

  const publishButton = article.querySelector('[data-ghrm-gist-publish]');
  publishButton?.addEventListener('click', () => {
    publish(article);
  });

  const input = article.querySelector('[data-ghrm-gist-form] textarea');
  input?.addEventListener('input', () => {
    syncBlob(article);
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

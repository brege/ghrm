import { showCopied, writeClipboard } from './adapters/copy.js';

const gistPath = '/_ghrm/gist';

let liveBound = false;

function currentArticle() {
  return document.querySelector('article[data-ghrm-gist]');
}

function currentText(article) {
  return (
    article?.querySelector('[data-ghrm-gist-current] .ghrm-data')?.content
      ?.textContent || ''
  );
}

function setStatus(article, message) {
  const status = article?.querySelector('[data-ghrm-gist-status]');
  if (status) {
    status.textContent = message;
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
    input.value = '';
    await refreshGist({ preserveInput: false });
  } catch {
    setStatus(article, 'Publish failed');
  } finally {
    if (button) {
      button.disabled = false;
    }
  }
}

async function refreshGist({ preserveInput }) {
  const article = currentArticle();
  if (!article) return;
  const input = article.querySelector('[data-ghrm-gist-form] textarea');
  const value = preserveInput ? input?.value || '' : '';
  const active = input && document.activeElement === input;
  const response = await fetch(gistPath, {
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
  const next = doc.querySelector('article[data-ghrm-gist]');
  if (!next) {
    setStatus(article, 'Refresh failed');
    return;
  }

  article.replaceWith(next);
  setupGist();
  const nextInput = next.querySelector('[data-ghrm-gist-form] textarea');
  if (nextInput && preserveInput) {
    nextInput.value = value;
    if (active) {
      nextInput.focus();
    }
  }
  document.dispatchEvent(new CustomEvent('ghrm:contentready'));
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

  const copy = article.querySelector('[data-ghrm-gist-copy]');
  copy?.addEventListener('click', async () => {
    await writeClipboard(currentText(article));
    showCopied(copy);
  });
}

function setupLiveGist() {
  if (liveBound) return;
  liveBound = true;
  document.addEventListener('ghrm:live:gist', () => {
    refreshGist({ preserveInput: true });
  });
}

document.addEventListener('DOMContentLoaded', () => {
  setupLiveGist();
  setupGist();
});

document.addEventListener('ghrm:contentready', setupGist);

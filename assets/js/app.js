import { checkIcon, copyIcon, showCopied, writeClipboard } from './preview.js';

const VALID_SCOPES = new Set(['filter', 'files', 'all']);

function defaultScope() {
  const scope = document.body?.dataset.defaultScope;
  return VALID_SCOPES.has(scope) ? scope : 'files';
}

function hasExtFilter() {
  return document.body?.dataset.hasExtFilter === '1';
}

function scrollOffset() {
  return 16;
}

function scrollToHash(hash) {
  if (!hash || hash === '#') return false;
  const id = decodeURIComponent(hash.slice(1));
  const target = document.getElementById(id);
  if (!target) return false;
  const top =
    window.scrollY + target.getBoundingClientRect().top - scrollOffset();
  window.scrollTo({ top: Math.max(top, 0), behavior: 'auto' });
  return true;
}

function currentScope() {
  const params = new URLSearchParams(location.search);
  const scope = params.get('scope');
  if (scope === 'filter' && !hasExtFilter()) return defaultScope();
  return VALID_SCOPES.has(scope) ? scope : defaultScope();
}

function withScope(urlLike, scope = currentScope()) {
  const url = new URL(urlLike, location.origin);
  if (scope === defaultScope()) {
    url.searchParams.delete('scope');
  } else {
    url.searchParams.set('scope', scope);
  }
  return `${url.pathname}${url.search}${url.hash}`;
}

function syncScopeSwitch() {
  const scope = currentScope();
  for (const button of document.querySelectorAll(
    '.ghrm-scope-option[data-scope]',
  )) {
    if (button.dataset.scope === 'filter' && !hasExtFilter()) {
      button.hidden = true;
      continue;
    }
    const active = button.dataset.scope === scope;
    button.classList.toggle('is-active', active);
    button.setAttribute('aria-pressed', active ? 'true' : 'false');
  }
}

function syncScopeVisibility() {
  for (const scopeSwitch of document.querySelectorAll('.ghrm-scope-switch')) {
    scopeSwitch.style.display = '';
  }
}

function formatRelative(ts) {
  const diff = Date.now() / 1000 - ts;
  const p = (n, u) => `${n} ${u}${n === 1 ? '' : 's'} ago`;
  if (diff < 60) return 'just now';
  if (diff < 3600) return p(Math.floor(diff / 60), 'minute');
  if (diff < 86400) return p(Math.floor(diff / 3600), 'hour');
  if (diff < 7 * 86400) return p(Math.floor(diff / 86400), 'day');
  if (diff < 30 * 86400) return p(Math.floor(diff / (7 * 86400)), 'week');
  if (diff < 365 * 86400) return p(Math.floor(diff / (30 * 86400)), 'month');
  return p(Math.floor(diff / (365 * 86400)), 'year');
}

function formatAbsolute(ts) {
  return new Date(ts * 1000).toLocaleString('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
    timeZoneName: 'short',
  });
}

function populateDates() {
  for (const el of document.querySelectorAll('.ghrm-nav-date[data-ts]')) {
    const ts = parseInt(el.dataset.ts, 10);
    if (!ts) continue;
    el.textContent = formatRelative(ts);
    el.title = formatAbsolute(ts);
  }
}

function setupScopeSwitch() {
  const buttons = document.querySelectorAll('.ghrm-scope-option[data-scope]');
  if (!buttons.length) return;

  syncScopeSwitch();
  for (const button of buttons) {
    button.addEventListener('click', () => {
      if (button.dataset.scope === 'filter' && !hasExtFilter()) {
        return;
      }
      const scope = VALID_SCOPES.has(button.dataset.scope)
        ? button.dataset.scope
        : defaultScope();
      if (scope === currentScope()) {
        return;
      }
      navigate(withScope(location.href, scope));
    });
  }
}

function setupThemeToggle() {
  const btn = document.getElementById('theme-toggle');
  if (!btn) return;
  btn.addEventListener('click', () => {
    const current = document.documentElement.getAttribute('data-theme');
    const next = current === 'dark' ? 'light' : 'dark';
    document.documentElement.setAttribute('data-theme', next);
    localStorage.setItem('ghrm-theme', next);
    document.dispatchEvent(
      new CustomEvent('ghrm:themechange', { detail: { theme: next } }),
    );
  });
}

function rawToggleIcon() {
  return `
    <svg aria-hidden="true" height="16" viewBox="0 0 640 512" width="16" class="ghrm-file-icon">
      <path d="M392.8 1.2c-17-4.9-34.7 5-39.6 22l-128 448c-4.9 17 5 34.7 22 39.6s34.7-5 39.6-22l128-448c4.9-17-5-34.7-22-39.6zm80.6 120.1c-12.5 12.5-12.5 32.8 0 45.3L562.7 256l-89.4 89.4c-12.5 12.5-12.5 32.8 0 45.3s32.8 12.5 45.3 0l112-112c12.5-12.5 12.5-32.8 0-45.3l-112-112c-12.5-12.5-32.8-12.5-45.3 0zm-306.7 0c-12.5-12.5-32.8-12.5-45.3 0l-112 112c-12.5 12.5-12.5 32.8 0 45.3l112 112c12.5 12.5 32.8 12.5 45.3 0s12.5-32.8 0-45.3L77.3 256l89.4-89.4c12.5-12.5 12.5-32.8 0-45.3z"></path>
    </svg>
  `;
}

function downloadIcon() {
  return `
    <svg aria-hidden="true" height="16" viewBox="0 0 16 16" width="16" class="ghrm-file-icon">
      <path d="M2.75 14A1.75 1.75 0 0 1 1 12.25v-2.5a.75.75 0 0 1 1.5 0v2.5c0 .138.112.25.25.25h10.5a.25.25 0 0 0 .25-.25v-2.5a.75.75 0 0 1 1.5 0v2.5A1.75 1.75 0 0 1 13.25 14Z"></path>
      <path d="M7.25 7.689V2a.75.75 0 0 1 1.5 0v5.689l1.97-1.969a.749.749 0 1 1 1.06 1.06l-3.25 3.25a.749.749 0 0 1-1.06 0L4.22 6.78a.749.749 0 1 1 1.06-1.06l1.97 1.969Z"></path>
    </svg>
  `;
}

function visiblePane(selector) {
  return document.querySelector(`${selector}:not([hidden])`);
}

function fileViewRoot() {
  return visiblePane('.ghrm-page-content [data-ghrm-preview-pane]');
}

function rawText(container) {
  return (
    container.querySelector('[data-ghrm-raw-pane] .ghrm-data')?.content
      ?.textContent ||
    container.querySelector('[data-ghrm-raw-pane] code')?.textContent ||
    ''
  );
}

function syncFileView(container, raw) {
  const preview = container.querySelector('[data-ghrm-preview-pane]');
  const rawPane = container.querySelector('[data-ghrm-raw-pane]');
  const toggle = container.querySelector('[data-ghrm-raw-toggle]');
  if (!preview || !rawPane || !toggle) return;

  preview.hidden = raw;
  rawPane.hidden = !raw;
  toggle.classList.toggle('is-active', raw);
  toggle.setAttribute('aria-pressed', raw ? 'true' : 'false');

  const label = raw ? 'Show preview' : 'Show raw';
  toggle.setAttribute('aria-label', label);
  toggle.title = label;
}

function fileActionsHost(container) {
  return container.querySelector('.ghrm-explorer-header .ghrm-header-actions');
}

function setupFileView(container) {
  const kind = container.dataset.ghrmViewKind;
  const rawUrl = container.dataset.ghrmRawUrl;
  const downloadUrl = container.dataset.ghrmDownloadUrl;
  const host = fileActionsHost(container);
  if (!kind || !rawUrl || !downloadUrl || !host) return;
  if (host.querySelector('.ghrm-file-tools')) return;

  const tools = document.createElement('div');
  tools.className = 'ghrm-file-tools';

  const toggle = document.createElement('button');
  toggle.type = 'button';
  toggle.className = 'ghrm-file-toggle';
  toggle.dataset.ghrmRawToggle = '1';
  toggle.innerHTML = rawToggleIcon();

  if (kind === 'markdown') {
    toggle.addEventListener('click', () => {
      const raw = toggle.getAttribute('aria-pressed') !== 'true';
      syncFileView(container, raw);
      buildToc();
      const panel = document.getElementById('ghrm-toc-panel');
      if (panel) {
        panel.hidden = true;
      }
    });
  } else {
    toggle.disabled = true;
  }

  const actions = document.createElement('div');
  actions.className = 'ghrm-file-actions';

  const rawLink = document.createElement('a');
  rawLink.className = 'ghrm-file-link';
  rawLink.href = rawUrl;
  rawLink.textContent = 'Raw';
  rawLink.target = '_blank';
  rawLink.rel = 'noopener noreferrer';
  rawLink.dataset.ghrmNative = '1';
  rawLink.setAttribute('aria-label', 'Open raw file');
  rawLink.title = 'Open raw file';

  const copy = document.createElement('button');
  copy.type = 'button';
  copy.className = 'ghrm-file-action';
  copy.innerHTML = `${copyIcon()}${checkIcon()}`;
  copy.dataset.copyLabel = 'Copy raw file';
  copy.dataset.copyFeedback = 'Copied!';
  copy.setAttribute('aria-label', 'Copy raw file');
  copy.title = 'Copy raw file';
  copy.addEventListener('click', async () => {
    await writeClipboard(rawText(container));
    showCopied(copy);
  });

  const download = document.createElement('a');
  download.className = 'ghrm-file-action';
  download.href = downloadUrl;
  download.dataset.ghrmNative = '1';
  download.setAttribute('download', '');
  download.setAttribute('aria-label', 'Download raw file');
  download.title = 'Download raw file';
  download.innerHTML = downloadIcon();

  actions.append(rawLink, copy, download);
  tools.append(toggle, actions);
  host.prepend(tools);
  syncFileView(container, kind === 'raw');
}

function setupFileViews() {
  for (const container of document.querySelectorAll(
    '.ghrm-page-shell[data-ghrm-view-kind]',
  )) {
    setupFileView(container);
  }
}

function hasDocChrome() {
  return !!document.querySelector('.ghrm-page-shell, .ghrm-readme-box');
}

function syncDocChromeToggle() {
  const btn = document.getElementById('doc-chrome-toggle');
  if (!btn) return;
  const show = hasDocChrome();
  btn.hidden = !show;
  if (!show) {
    btn.removeAttribute('title');
    btn.removeAttribute('aria-label');
    return;
  }
  const flat = document.body.classList.contains('ghrm-doc-flat');
  const label = flat ? 'Show document wrapper' : 'Hide document wrapper';
  btn.title = label;
  btn.setAttribute('aria-label', label);
}

function applyDocChromePref() {
  const flat = localStorage.getItem('ghrm-doc-flat') === '1';
  document.body.classList.toggle('ghrm-doc-flat', flat && hasDocChrome());
  syncDocChromeToggle();
}

function setupDocChromeToggle() {
  const btn = document.getElementById('doc-chrome-toggle');
  if (!btn) return;
  btn.addEventListener('click', () => {
    const next = !document.body.classList.contains('ghrm-doc-flat');
    document.body.classList.toggle('ghrm-doc-flat', next && hasDocChrome());
    localStorage.setItem('ghrm-doc-flat', next ? '1' : '0');
    syncDocChromeToggle();
  });
  applyDocChromePref();
}

function setupLiveReload() {
  const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  const url = `${proto}//${location.host}/_ghrm/ws`;
  function connect() {
    const ws = new WebSocket(url);
    ws.onmessage = (ev) => {
      if (ev.data === 'reload') location.reload();
    };
    ws.onclose = () => {
      setTimeout(connect, 1000);
    };
  }
  connect();
}

function setupSpaNav() {
  document.addEventListener('click', (e) => {
    const a = e.target.closest('a');
    if (!a || !a.href) return;
    if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey)
      return;
    if (a.dataset.ghrmNative === '1') return;
    if (a.target && a.target !== '_self') return;
    if (a.origin !== location.origin) return;
    if (a.pathname === location.pathname && a.hash) return;

    const { pathname } = a;
    if (!pathname.endsWith('/') && !pathname.endsWith('.md')) return;

    e.preventDefault();
    navigate(withScope(a.href));
  });

  window.addEventListener('popstate', () => {
    const target = `${location.pathname}${location.search}${location.hash}`;
    navigate(target, false);
  });
}

async function navigate(path, push = true) {
  const url = new URL(path, location.origin);
  const target = `${url.pathname}${url.search}${url.hash}`;
  const res = await fetch(target).catch(() => null);
  if (!res || !res.ok) return;

  const html = await res.text();
  const doc = new DOMParser().parseFromString(html, 'text/html');
  const newArticle = doc.querySelector('article.markdown-body');
  if (!newArticle) return;
  const nextSource = doc.getElementById('ghrm-source-slot');
  const currentSource = document.getElementById('ghrm-source-slot');
  if (currentSource && nextSource) {
    currentSource.replaceWith(nextSource);
  } else if (currentSource) {
    currentSource.remove();
  } else if (nextSource) {
    document.querySelector('.ghrm-topbar-inner')?.prepend(nextSource);
  }

  const existing = document.querySelector('article.markdown-body');
  if (existing) {
    existing.replaceWith(newArticle);
  } else {
    document.body.appendChild(newArticle);
  }

  document.title = doc.title;
  if (push) history.pushState(null, '', target);
  setupFileViews();
  setupScopeSwitch();
  syncScopeSwitch();
  syncScopeVisibility();
  applyDocChromePref();
  populateDates();
  buildToc();
  const hash = url.hash;
  if (!hash || !scrollToHash(hash)) {
    window.scrollTo(0, 0);
  }
  document.dispatchEvent(new CustomEvent('ghrm:contentready'));
}

function tocRoot() {
  const viewRoot = fileViewRoot();
  if (viewRoot) return viewRoot;
  if (document.querySelector('[data-ghrm-view-kind]')) return null;
  return (
    document.querySelector('article[data-explorer] .ghrm-readme-content') ||
    document.querySelector('article.markdown-body')
  );
}

function tocButton() {
  return document.querySelector('[data-ghrm-toc-btn]');
}

function syncTocButtons(show) {
  const btn = tocButton();
  for (const current of document.querySelectorAll('[data-ghrm-toc-btn]')) {
    current.hidden = current !== btn;
    current.disabled = current === btn ? !show : true;
  }
  return btn;
}

function headingText(heading) {
  const copy = heading.cloneNode(true);
  for (const anchor of copy.querySelectorAll('.ghrm-anchor')) {
    anchor.remove();
  }
  return copy.textContent.replace(/\s+/g, ' ').trim();
}

function currentHeadingId() {
  const root = tocRoot();
  if (!root) return '';
  const headings = [
    ...root.querySelectorAll('h1[id],h2[id],h3[id],h4[id],h5[id],h6[id]'),
  ];
  if (headings.length === 0) return '';

  const threshold = scrollOffset() + 12;
  let current = headings[0];
  for (const heading of headings) {
    if (window.scrollY + heading.getBoundingClientRect().top <= threshold) {
      current = heading;
    } else {
      break;
    }
  }
  return current.id;
}

function syncTocActive() {
  const panel = document.getElementById('ghrm-toc-panel');
  if (!panel) return;
  const activeId = currentHeadingId();
  for (const link of panel.querySelectorAll('a[href^="#"]')) {
    const href = decodeURIComponent(link.getAttribute('href') ?? '').slice(1);
    const active = href === activeId;
    link.classList.toggle('is-active', active);
    if (active) {
      link.setAttribute('aria-current', 'location');
    } else {
      link.removeAttribute('aria-current');
    }
  }
}

function buildToc() {
  const panel = document.getElementById('ghrm-toc-panel');
  if (!panel) return;

  panel.hidden = true;
  panel.replaceChildren();

  const root = tocRoot();
  const headings = root
    ? [...root.querySelectorAll('h1[id],h2[id],h3[id],h4[id],h5[id],h6[id]')]
    : [];

  if (headings.length === 0) {
    syncTocButtons(false);
    return;
  }

  syncTocButtons(true);
  for (const heading of headings) {
    const text = headingText(heading);
    if (!text) continue;
    const link = document.createElement('a');
    link.className = `toc-h${heading.tagName[1]}`;
    link.href = `#${heading.id}`;
    link.textContent = text;
    panel.append(link);
  }
  syncTocActive();
}

function positionToc(panel, btn) {
  const rect = btn.getBoundingClientRect();
  const width = panel.offsetWidth || 248;
  const left = Math.max(
    16,
    Math.min(rect.right - width, window.innerWidth - width - 16),
  );
  panel.style.top = `${Math.round(rect.bottom + 8)}px`;
  panel.style.left = `${Math.round(left)}px`;
}

function setupToc() {
  const panel = document.getElementById('ghrm-toc-panel');
  if (!panel) return;

  panel.addEventListener('click', (e) => {
    if (e.target.tagName === 'A') panel.hidden = true;
  });

  document.addEventListener('click', (e) => {
    const btn = e.target.closest('[data-ghrm-toc-btn]');
    if (btn) {
      if (btn.hidden || btn.disabled) return;
      buildToc();
      const nextHidden = !panel.hidden;
      panel.hidden = nextHidden;
      if (!nextHidden && panel.childElementCount > 0) {
        positionToc(panel, btn);
      }
      return;
    }
    if (!panel.contains(e.target)) {
      panel.hidden = true;
    }
  });

  window.addEventListener('resize', () => {
    if (panel.hidden) return;
    const btn = tocButton();
    if (btn) positionToc(panel, btn);
  });

  window.addEventListener('hashchange', () => {
    panel.hidden = true;
    scrollToHash(location.hash);
    syncTocActive();
  });

  window.addEventListener('scroll', syncTocActive, { passive: true });

  buildToc();
}

document.addEventListener('DOMContentLoaded', () => {
  setupFileViews();
  setupScopeSwitch();
  syncScopeVisibility();
  setupDocChromeToggle();
  populateDates();
  setupToc();
  setupThemeToggle();
  setupLiveReload();
  setupSpaNav();
  scrollToHash(location.hash);
});

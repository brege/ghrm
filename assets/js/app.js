const DEFAULT_SCOPE = 'md';
const VALID_SCOPES = new Set(['md', 'files', 'all']);

function logScope(event, detail = {}) {
  console.log('[ghrm scope]', event, detail);
}

function currentScope() {
  const params = new URLSearchParams(location.search);
  const scope = params.get('scope');
  return VALID_SCOPES.has(scope) ? scope : DEFAULT_SCOPE;
}

function withScope(urlLike, scope = currentScope()) {
  const url = new URL(urlLike, location.origin);
  if (scope === DEFAULT_SCOPE) {
    url.searchParams.delete('scope');
  } else {
    url.searchParams.set('scope', scope);
  }
  return `${url.pathname}${url.search}${url.hash}`;
}

function syncScopeSwitch() {
  const scope = currentScope();
  for (const button of document.querySelectorAll('.ghrm-scope-option[data-scope]')) {
    const active = button.dataset.scope === scope;
    button.classList.toggle('is-active', active);
    button.setAttribute('aria-pressed', active ? 'true' : 'false');
  }
  logScope('sync-switch', { scope, href: location.href });
}

function setupScopeSwitch() {
  const buttons = document.querySelectorAll('.ghrm-scope-option[data-scope]');
  if (!buttons.length) return;

  syncScopeSwitch();
  for (const button of buttons) {
    button.addEventListener('click', () => {
      const scope = VALID_SCOPES.has(button.dataset.scope) ? button.dataset.scope : DEFAULT_SCOPE;
      const current = currentScope();
      logScope('click', {
        buttonScope: scope,
        currentScope: current,
        href: location.href,
      });
      if (scope === current) {
        logScope('click-noop', { scope, reason: 'already-active' });
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
    document.dispatchEvent(new CustomEvent('ghrm:themechange', { detail: { theme: next } }));
  });
}

function setupLiveReload() {
  const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  const url = `${proto}//${location.host}/_ghrm/ws`;
  function connect() {
    const ws = new WebSocket(url);
    ws.onmessage = (ev) => { if (ev.data === 'reload') location.reload(); };
    ws.onclose = () => { setTimeout(connect, 1000); };
  }
  connect();
}

function setupSpaNav() {
  document.addEventListener('click', (e) => {
    const a = e.target.closest('a');
    if (!a || !a.href) return;
    if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    if (a.target && a.target !== '_self') return;
    if (a.origin !== location.origin) return;
    if (a.pathname === location.pathname && a.hash) return;

    const { pathname } = a;
    if (!pathname.endsWith('/') && !pathname.endsWith('.md')) return;

    e.preventDefault();
    logScope('link-nav', {
      href: a.href,
      target: withScope(a.href),
      scope: currentScope(),
    });
    navigate(withScope(a.href));
  });

  window.addEventListener('popstate', () => {
    const target = `${location.pathname}${location.search}${location.hash}`;
    logScope('popstate', { target, scope: currentScope() });
    navigate(target, false);
  });
}

async function navigate(path, push = true) {
  const url = new URL(path, location.origin);
  const target = `${url.pathname}${url.search}${url.hash}`;
  logScope('navigate-start', { path, target, push, scope: currentScope() });
  const res = await fetch(target).catch(() => null);
  if (!res) {
    logScope('navigate-fail', { target, reason: 'network-error' });
    return;
  }
  if (!res.ok) {
    logScope('navigate-fail', { target, reason: 'bad-status', status: res.status });
    return;
  }

  const html = await res.text();
  const doc = new DOMParser().parseFromString(html, 'text/html');
  const newArticle = doc.querySelector('article.markdown-body');
  if (!newArticle) {
    logScope('navigate-fail', { target, reason: 'missing-article' });
    return;
  }

  const existing = document.querySelector('article.markdown-body');
  if (existing) {
    existing.replaceWith(newArticle);
  } else {
    document.body.appendChild(newArticle);
  }

  document.title = doc.title;
  if (push) history.pushState(null, '', target);
  syncScopeSwitch();
  logScope('navigate-done', {
    target,
    title: doc.title,
    scope: currentScope(),
  });
  const hash = url.hash;
  if (hash) {
    document.querySelector(hash)?.scrollIntoView();
  } else {
    window.scrollTo(0, 0);
  }
  document.dispatchEvent(new CustomEvent('ghrm:contentready'));
}

document.addEventListener('DOMContentLoaded', () => {
  logScope('boot', { href: location.href, scope: currentScope() });
  setupScopeSwitch();
  setupThemeToggle();
  setupLiveReload();
  setupSpaNav();
});

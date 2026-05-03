import {
  applyDocChromePref,
  setupDocChromeToggle,
  setupThemeToggle,
} from './chrome.js';
import {
  icon,
  isHtmlFile,
  positionFloatingPanel,
  scrollToHash,
} from './dom.js';
import { checkIcon, copyIcon, showCopied, writeClipboard } from './preview.js';
import {
  refreshActiveSearch,
  setSearchCloseHandler,
  setupPathSearch,
} from './search.js';
import {
  beginActivity,
  endActivity,
  setConnected,
  setupStatusPeek,
  syncServerStatus,
} from './status.js';
import { buildToc, setupToc } from './toc.js';

let explorerMenusBound = false;
let pendingSamePathSwap = false;

const EXPLORER_MENUS = [
  {
    name: 'filter',
    toggleId: 'ghrm-view-menu-toggle',
    panelId: 'ghrm-view-menu',
  },
  {
    name: 'sort',
    toggleId: 'ghrm-sort-menu-toggle',
    panelId: 'ghrm-sort-menu',
  },
  {
    name: 'column',
    toggleId: 'ghrm-column-menu-toggle',
    panelId: 'ghrm-column-menu',
  },
];

function syncColumnControls() {
  const article = document.querySelector('article[data-explorer]');
  const controls = [
    ...document.querySelectorAll('[data-column-toggle].ghrm-view-option'),
  ];
  const columns = new Set(
    controls
      .filter((control) => {
        return (
          control.dataset.columnToggle !== 'headers' &&
          control.classList.contains('is-active')
        );
      })
      .map((control) => control.dataset.columnToggle),
  );
  if (article) {
    const hasEdge = controls.some((control) => {
      return (
        control.dataset.columnToggle !== 'headers' &&
        control.dataset.columnEdge === '1' &&
        control.classList.contains('is-active')
      );
    });
    article.classList.toggle('ghrm-has-edge-meta', hasEdge);
    for (const cell of article.querySelectorAll('[data-column-key]')) {
      cell.hidden = !columns.has(cell.dataset.columnKey);
    }
    const headers = article.querySelector('.ghrm-column-headers');
    const headerControl = controls.find((control) => {
      return control.dataset.columnToggle === 'headers';
    });
    if (headers) {
      headers.hidden = !headerControl?.classList.contains('is-active');
    }
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
  for (const el of document.querySelectorAll('.ghrm-nav-meta-time[data-ts]')) {
    const ts = parseInt(el.dataset.ts, 10);
    if (!ts) continue;
    el.textContent = formatRelative(ts);
    el.title = formatAbsolute(ts);
  }
}

function closeExplorerMenus() {
  for (const { toggle, panel } of currentExplorerMenus()) {
    panel.hidden = true;
    toggle.setAttribute('aria-expanded', 'false');
  }
}

function currentExplorerMenus() {
  return EXPLORER_MENUS.map((menu) => ({
    ...menu,
    toggle: document.getElementById(menu.toggleId),
    panel: document.getElementById(menu.panelId),
  })).filter(({ toggle, panel }) => toggle && panel);
}

function currentExplorerMenu(name) {
  return currentExplorerMenus().find((menu) => menu.name === name) || null;
}

function hasExplorerMenus() {
  return currentExplorerMenus().length === EXPLORER_MENUS.length;
}

function openExplorerMenu(name) {
  const menu = currentExplorerMenu(name);
  if (!menu) return;
  closeExplorerMenus();
  menu.panel.hidden = false;
  menu.toggle.setAttribute('aria-expanded', 'true');
  positionFloatingPanel(menu.panel, menu.toggle);
}

function setupViewMenu() {
  const filter = currentExplorerMenu('filter');
  const sort = currentExplorerMenu('sort');
  const column = currentExplorerMenu('column');
  if (!filter || !sort || !column) return;

  syncColumnControls();
  closeExplorerMenus();

  for (const menu of currentExplorerMenus()) {
    menu.toggle.onclick = () => {
      if (menu.panel.hidden) {
        openExplorerMenu(menu.name);
      } else {
        closeExplorerMenus();
      }
    };
    for (const option of menu.panel.querySelectorAll('.ghrm-view-option')) {
      option.onclick = () => {
        closeExplorerMenus();
      };
    }
  }

  if (explorerMenusBound) {
    return;
  }
  explorerMenusBound = true;

  document.addEventListener('click', (e) => {
    const dirToggle = document.getElementById('ghrm-sort-dir-toggle');
    if (!hasExplorerMenus()) return;
    const insideMenu = currentExplorerMenus().some(({ toggle, panel }) => {
      return toggle.contains(e.target) || panel.contains(e.target);
    });
    if (insideMenu || dirToggle?.contains(e.target)) return;
    closeExplorerMenus();
  });

  window.addEventListener('resize', () => {
    if (!hasExplorerMenus()) return;
    for (const { toggle, panel } of currentExplorerMenus()) {
      if (!panel.hidden) {
        positionFloatingPanel(panel, toggle);
      }
    }
  });

  document.addEventListener('keydown', (e) => {
    if (e.key !== 'Escape') return;
    if (!hasExplorerMenus()) return;
    const openMenu = currentExplorerMenus().find(({ panel }) => !panel.hidden);
    if (openMenu) {
      closeExplorerMenus();
      openMenu.toggle.focus();
    }
  });
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

  syncWrapToggle(container, raw);
}

function getWrapPref() {
  return localStorage.getItem('ghrm-wrap') === '1';
}

function setWrapPref(wrap) {
  localStorage.setItem('ghrm-wrap', wrap ? '1' : '0');
}

function applyWrapState(wrap) {
  document.body.classList.toggle('ghrm-wrap', wrap);
}

function syncWrapToggle(container, isRaw) {
  const wrapToggle = container.querySelector('[data-ghrm-wrap-toggle]');
  if (!wrapToggle) return;

  const disabled = !isRaw;
  wrapToggle.disabled = disabled;

  if (disabled) {
    wrapToggle.classList.remove('is-active');
    wrapToggle.setAttribute('aria-pressed', 'false');
    wrapToggle.setAttribute('aria-label', 'Wrap lines (code view only)');
    wrapToggle.title = 'Wrap lines (code view only)';
    applyWrapState(false);
  } else {
    const wrap = getWrapPref();
    wrapToggle.classList.toggle('is-active', wrap);
    wrapToggle.setAttribute('aria-pressed', wrap ? 'true' : 'false');
    const label = wrap ? 'Disable line wrap' : 'Wrap lines';
    wrapToggle.setAttribute('aria-label', label);
    wrapToggle.title = label;
    applyWrapState(wrap);
  }
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

  const toggles = document.createElement('div');
  toggles.className = 'ghrm-file-toggles';

  const toggle = document.createElement('button');
  toggle.type = 'button';
  toggle.className = 'ghrm-file-toggle';
  toggle.dataset.ghrmRawToggle = '1';
  toggle.innerHTML = icon('code');

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

  const wrapToggle = document.createElement('button');
  wrapToggle.type = 'button';
  wrapToggle.className = 'ghrm-file-toggle';
  wrapToggle.dataset.ghrmWrapToggle = '1';
  wrapToggle.innerHTML = icon('wrap');

  wrapToggle.addEventListener('click', () => {
    setWrapPref(!getWrapPref());
    syncWrapToggle(container, true);
  });

  toggles.append(toggle, wrapToggle);

  if (isHtmlFile(rawUrl)) {
    const htmlUrl = rawUrl.replace('/_ghrm/raw/', '/_ghrm/html/');
    const external = document.createElement('a');
    external.className = 'ghrm-file-toggle';
    external.href = htmlUrl;
    external.target = '_blank';
    external.rel = 'noopener noreferrer';
    external.dataset.ghrmNative = '1';
    external.setAttribute('aria-label', 'Open in browser');
    external.title = 'Open in browser';
    external.innerHTML = icon('external');
    toggles.append(external);
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
  download.innerHTML = icon('download');

  actions.append(rawLink, copy, download);
  tools.append(toggles, actions);
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

function setupLiveReload() {
  const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  const url = `${proto}//${location.host}/_ghrm/ws`;
  let connectedOnce = false;
  function connect() {
    const ws = new WebSocket(url);
    ws.onopen = () => {
      setConnected(true);
      if (connectedOnce) {
        location.reload();
        return;
      }
      connectedOnce = true;
    };
    ws.onmessage = (ev) => {
      if (ev.data === 'reload') {
        location.reload();
      } else if (ev.data === 'nav-ready') {
        refreshActiveSearch();
      }
    };
    ws.onerror = () => {
      setConnected(false);
    };
    ws.onclose = () => {
      setConnected(false);
      setTimeout(connect, 1000);
    };
  }
  connect();
}

function setupSearch() {
  setSearchCloseHandler(() => {
    const target = `${location.pathname}${location.search}${location.hash}`;
    location.assign(target);
  });
  setupPathSearch({ populateDates, setupNavExternalLinks, syncColumnControls });
}

function setupNavExternalLinks() {
  for (const row of document.querySelectorAll('.ghrm-nav-table tr')) {
    const nameLink = row.querySelector('.ghrm-nav-name a');
    const nameCell = nameLink?.closest('.ghrm-nav-name');
    if (!nameLink || !nameCell) continue;

    const href = nameLink.getAttribute('href');
    if (!isHtmlFile(href)) continue;
    if (nameCell.querySelector('.ghrm-nav-external')) continue;

    const htmlHref = href.replace(/^\//, '/_ghrm/html/');
    const link = document.createElement('a');
    link.className = 'ghrm-nav-external';
    link.href = htmlHref;
    link.target = '_blank';
    link.rel = 'noopener noreferrer';
    link.dataset.ghrmNative = '1';
    link.setAttribute('aria-label', 'Open in browser');
    link.title = 'Open in browser';
    link.innerHTML = icon('external');
    nameLink.after(link);
  }
}

function shouldBoostLink(a) {
  if (!a.href) return false;
  if (a.dataset.ghrmNative === '1') return false;
  if (a.target && a.target !== '_self') return false;
  if (a.hasAttribute('download')) return false;
  const url = new URL(a.href, location.origin);
  if (url.origin !== location.origin) return false;
  if (url.pathname.startsWith('/_ghrm/')) return false;
  if (url.pathname === location.pathname && url.hash) return false;
  return true;
}

function setupHtmxNav() {
  document.body.addEventListener('htmx:beforeBoost', (e) => {
    const link = e.detail.elt?.closest?.('a');
    if (link && !shouldBoostLink(link)) {
      e.preventDefault();
    }
  });

  document.body.addEventListener('htmx:afterSwap', (e) => {
    if (e.detail.target?.matches('article.markdown-body')) {
      const title = e.detail.xhr?.getResponseHeader('HX-Title');
      if (title !== null) {
        document.title = decodeURIComponent(title);
      }
      syncServerStatus();
      setupFileViews();
      setupSearch();
      setupNavExternalLinks();
      setupViewMenu();
      syncColumnControls();
      applyDocChromePref();
      populateDates();
      buildToc();
      const hash = location.hash;
      if (hash) {
        scrollToHash(hash);
      } else if (!pendingSamePathSwap) {
        window.scrollTo(0, 0);
      }
      pendingSamePathSwap = false;
      document.dispatchEvent(new CustomEvent('ghrm:contentready'));
    }
  });

  document.body.addEventListener('htmx:beforeRequest', (e) => {
    if (e.detail.target?.matches('article.markdown-body')) {
      const link = e.detail.elt?.closest?.('a');
      pendingSamePathSwap = link
        ? new URL(link.href, location.origin).pathname === location.pathname
        : false;
      beginActivity();
    }
  });

  document.body.addEventListener('htmx:afterRequest', (e) => {
    if (e.detail.target?.matches('article.markdown-body')) {
      endActivity();
    }
  });

  document.body.addEventListener('htmx:afterSettle', () => {
    syncServerStatus();
  });
}

document.addEventListener('DOMContentLoaded', () => {
  setupFileViews();
  setupSearch();
  setupViewMenu();
  setupDocChromeToggle();
  populateDates();
  setupToc();
  setupThemeToggle();
  setupStatusPeek();
  setupLiveReload();
  setupHtmxNav();
  setupNavExternalLinks();
  scrollToHash(location.hash);
});

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
import { setupPathSearch } from './search.js';
import { buildToc, setupToc } from './toc.js';
import {
  canToggleExcludes,
  currentView,
  defaultFilterExt,
  defaultFilterGroups,
  defaultShowExcludes,
  defaultShowHidden,
  defaultSort,
  defaultSortDir,
  withView,
} from './view.js';

let explorerMenusBound = false;
let reopenFilterMenu = false;

function syncViewMenu() {
  const view = currentView();
  const toggle = document.getElementById('ghrm-view-menu-toggle');
  if (toggle) {
    const active =
      view.showHidden !== defaultShowHidden() ||
      (canToggleExcludes() && view.showExcludes !== defaultShowExcludes()) ||
      view.filterExt !== defaultFilterExt() ||
      view.filterGroups.join(',') !== defaultFilterGroups().join(',');
    toggle.classList.toggle('is-active', active);
  }
  for (const button of document.querySelectorAll(
    '#ghrm-view-menu .ghrm-view-option',
  )) {
    if (button.dataset.viewToggle === 'excludes' && !canToggleExcludes()) {
      button.hidden = true;
      continue;
    }
    const active =
      (button.dataset.viewToggle === 'hidden' && view.showHidden) ||
      (button.dataset.viewToggle === 'excludes' && view.showExcludes) ||
      (button.dataset.viewToggle === 'filter' && view.filterExt) ||
      (button.dataset.filterGroup &&
        view.filterGroups.includes(button.dataset.filterGroup));
    button.classList.toggle('is-active', active);
    button.setAttribute('aria-checked', active ? 'true' : 'false');
  }
}

function syncSortControls() {
  const view = currentView();
  const sortToggle = document.getElementById('ghrm-sort-menu-toggle');
  if (sortToggle) {
    const active =
      view.sort !== defaultSort() || view.sortDir !== defaultSortDir(view.sort);
    sortToggle.classList.toggle('is-active', active);
  }
  for (const button of document.querySelectorAll(
    '#ghrm-sort-menu .ghrm-view-option',
  )) {
    const active = button.dataset.sort === view.sort;
    button.classList.toggle('is-active', active);
    button.setAttribute('aria-checked', active ? 'true' : 'false');
  }
  const dirToggle = document.getElementById('ghrm-sort-dir-toggle');
  if (!dirToggle) return;
  const use = dirToggle.querySelector('use');
  if (use) {
    use.setAttribute(
      'href',
      view.sortDir === 'desc'
        ? '#ghrm-icon-chevron-down'
        : '#ghrm-icon-chevron-up',
    );
  }
  const label = view.sortDir === 'desc' ? 'Sort descending' : 'Sort ascending';
  dirToggle.title = label;
  dirToggle.setAttribute('aria-label', label);
  dirToggle.classList.toggle(
    'is-active',
    view.sortDir !== defaultSortDir(view.sort),
  );
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

function closeExplorerMenus() {
  for (const [toggleId, panelId] of [
    ['ghrm-view-menu-toggle', 'ghrm-view-menu'],
    ['ghrm-sort-menu-toggle', 'ghrm-sort-menu'],
  ]) {
    const toggle = document.getElementById(toggleId);
    const panel = document.getElementById(panelId);
    if (!toggle || !panel) continue;
    panel.hidden = true;
    toggle.setAttribute('aria-expanded', 'false');
  }
}

function openExplorerMenu(toggle, panel) {
  closeExplorerMenus();
  panel.hidden = false;
  toggle.setAttribute('aria-expanded', 'true');
  positionFloatingPanel(panel, toggle);
}

function currentExplorerControls() {
  return {
    filterToggle: document.getElementById('ghrm-view-menu-toggle'),
    filterPanel: document.getElementById('ghrm-view-menu'),
    sortToggle: document.getElementById('ghrm-sort-menu-toggle'),
    sortPanel: document.getElementById('ghrm-sort-menu'),
    dirToggle: document.getElementById('ghrm-sort-dir-toggle'),
  };
}

function setupViewMenu() {
  const filterToggle = document.getElementById('ghrm-view-menu-toggle');
  const filterPanel = document.getElementById('ghrm-view-menu');
  const sortToggle = document.getElementById('ghrm-sort-menu-toggle');
  const sortPanel = document.getElementById('ghrm-sort-menu');
  const dirToggle = document.getElementById('ghrm-sort-dir-toggle');
  if (!filterToggle || !filterPanel || !sortToggle || !sortPanel || !dirToggle)
    return;

  syncViewMenu();
  syncSortControls();
  filterPanel.hidden = true;
  sortPanel.hidden = true;
  filterToggle.setAttribute('aria-expanded', 'false');
  sortToggle.setAttribute('aria-expanded', 'false');

  filterToggle.onclick = () => {
    if (filterPanel.hidden) {
      openExplorerMenu(filterToggle, filterPanel);
    } else {
      closeExplorerMenus();
    }
  };

  sortToggle.onclick = () => {
    if (sortPanel.hidden) {
      openExplorerMenu(sortToggle, sortPanel);
    } else {
      closeExplorerMenus();
    }
  };

  dirToggle.onclick = () => {
    const view = currentView();
    const next = {
      ...view,
      filterGroups: [...view.filterGroups],
      sortDir: view.sortDir === 'desc' ? 'asc' : 'desc',
    };
    navigate(withView(location.href, next));
  };

  for (const button of filterPanel.querySelectorAll('.ghrm-view-option')) {
    button.onclick = () => {
      const view = currentView();
      const next = {
        ...view,
        filterGroups: [...view.filterGroups],
      };
      switch (button.dataset.viewToggle) {
        case 'hidden':
          next.showHidden = !view.showHidden;
          break;
        case 'excludes':
          if (!canToggleExcludes()) return;
          next.showExcludes = !view.showExcludes;
          break;
        case 'filter':
          next.filterExt = !view.filterExt;
          if (next.filterExt && next.filterGroups.length === 0) {
            next.filterGroups = defaultFilterGroups();
          }
          break;
        default:
          if (button.dataset.filterGroup) {
            const group = button.dataset.filterGroup;
            next.filterExt = true;
            if (next.filterGroups.includes(group)) {
              next.filterGroups = next.filterGroups.filter(
                (current) => current !== group,
              );
              if (next.filterGroups.length === 0) {
                next.filterExt = false;
              }
            } else {
              next.filterGroups.push(group);
              next.filterGroups = [...new Set(next.filterGroups)];
            }
            break;
          }
          return;
      }
      reopenFilterMenu = true;
      navigate(withView(location.href, next));
    };
  }

  for (const button of sortPanel.querySelectorAll('.ghrm-view-option')) {
    button.onclick = () => {
      const view = currentView();
      const next = {
        ...view,
        filterGroups: [...view.filterGroups],
        sort: button.dataset.sort || view.sort,
      };
      if (!button.dataset.sort) return;
      if (!new URLSearchParams(location.search).has('dir')) {
        next.sortDir = defaultSortDir(next.sort);
      }
      closeExplorerMenus();
      navigate(withView(location.href, next));
    };
  }

  if (explorerMenusBound) {
    if (reopenFilterMenu) {
      reopenFilterMenu = false;
      openExplorerMenu(filterToggle, filterPanel);
    }
    return;
  }
  explorerMenusBound = true;

  document.addEventListener('click', (e) => {
    const { filterToggle, filterPanel, sortToggle, sortPanel, dirToggle } =
      currentExplorerControls();
    if (
      !filterToggle ||
      !filterPanel ||
      !sortToggle ||
      !sortPanel ||
      !dirToggle
    ) {
      return;
    }
    const insideFilter =
      filterToggle.contains(e.target) || filterPanel.contains(e.target);
    const insideSort =
      sortToggle.contains(e.target) || sortPanel.contains(e.target);
    const insideDir = dirToggle.contains(e.target);
    if (insideFilter || insideSort || insideDir) return;
    closeExplorerMenus();
  });

  window.addEventListener('resize', () => {
    const { filterToggle, filterPanel, sortToggle, sortPanel } =
      currentExplorerControls();
    if (!filterToggle || !filterPanel || !sortToggle || !sortPanel) return;
    if (!filterPanel.hidden) {
      positionFloatingPanel(filterPanel, filterToggle);
    }
    if (!sortPanel.hidden) {
      positionFloatingPanel(sortPanel, sortToggle);
    }
  });

  document.addEventListener('keydown', (e) => {
    if (e.key !== 'Escape') return;
    const { filterToggle, filterPanel, sortToggle, sortPanel } =
      currentExplorerControls();
    if (!filterToggle || !filterPanel || !sortToggle || !sortPanel) return;
    if (!filterPanel.hidden) {
      closeExplorerMenus();
      filterToggle.focus();
      return;
    }
    if (!sortPanel.hidden) {
      closeExplorerMenus();
      sortToggle.focus();
    }
  });

  if (reopenFilterMenu) {
    reopenFilterMenu = false;
    openExplorerMenu(filterToggle, filterPanel);
  }
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
    navigate(withView(a.href));
  });

  window.addEventListener('popstate', () => {
    const target = `${location.pathname}${location.search}${location.hash}`;
    navigate(target, false);
  });
}

function setupSearch() {
  setupPathSearch({ populateDates, setupNavExternalLinks });
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
  setupSearch();
  setupNavExternalLinks();
  setupViewMenu();
  syncViewMenu();
  applyDocChromePref();
  populateDates();
  buildToc();
  const hash = url.hash;
  if (!hash || !scrollToHash(hash)) {
    window.scrollTo(0, 0);
  }
  document.dispatchEvent(new CustomEvent('ghrm:contentready'));
}

function setupNavExternalLinks() {
  for (const row of document.querySelectorAll('.ghrm-nav-table tr')) {
    const iconCell = row.querySelector('.ghrm-nav-icon');
    const nameLink = row.querySelector('.ghrm-nav-name a');
    if (!iconCell || !nameLink) continue;

    const href = nameLink.getAttribute('href');
    if (!isHtmlFile(href)) continue;
    if (iconCell.querySelector('a')) continue;

    const htmlHref = href.replace(/^\//, '/_ghrm/html/');
    const svg = iconCell.querySelector('svg');
    if (!svg) continue;

    const use = svg.querySelector('use');
    if (use) {
      use.setAttribute('href', '#ghrm-icon-external');
    }

    const link = document.createElement('a');
    link.href = htmlHref;
    link.target = '_blank';
    link.rel = 'noopener noreferrer';
    link.dataset.ghrmNative = '1';
    link.setAttribute('aria-label', 'Open in browser');
    link.title = 'Open in browser';
    link.appendChild(svg);
    iconCell.appendChild(link);
  }
}

document.addEventListener('DOMContentLoaded', () => {
  setupFileViews();
  setupSearch();
  setupViewMenu();
  syncViewMenu();
  setupDocChromeToggle();
  populateDates();
  setupToc();
  setupThemeToggle();
  setupLiveReload();
  setupSpaNav();
  setupNavExternalLinks();
  scrollToHash(location.hash);
});

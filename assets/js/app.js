import { checkIcon, copyIcon, showCopied, writeClipboard } from './preview.js';

let explorerMenusBound = false;
let reopenFilterMenu = false;
let searchMode = 'path';

function defaultShowHidden() {
  return document.body?.dataset.defaultShowHidden === '1';
}

function defaultShowExcludes() {
  return document.body?.dataset.defaultShowExcludes === '1';
}

function defaultFilterExt() {
  return document.body?.dataset.defaultFilterExt === '1';
}

function defaultFilterGroup() {
  return document.body?.dataset.defaultFilterGroup || null;
}

function defaultFilterGroups() {
  const group = defaultFilterGroup();
  return group ? [group] : [];
}

function defaultSort() {
  return document.body?.dataset.defaultSort || 'name';
}

function defaultSortDir(sort = defaultSort()) {
  return sort === 'timestamp' ? 'desc' : 'asc';
}

function canToggleExcludes() {
  return document.body?.dataset.canToggleExcludes === '1';
}

function parseQueryBool(raw) {
  if (raw === '1' || raw === 'true') return true;
  if (raw === '0' || raw === 'false') return false;
  return null;
}

function parseSort(raw) {
  switch (raw) {
    case 'name':
    case 'type':
    case 'timestamp':
      return raw;
    default:
      return null;
  }
}

function parseSortDir(raw) {
  switch (raw) {
    case 'asc':
    case 'desc':
      return raw;
    default:
      return null;
  }
}

function scrollOffset() {
  return 16;
}

function positionFloatingPanel(panel, button, fallbackWidth = 220) {
  const rect = button.getBoundingClientRect();
  const width = panel.offsetWidth || fallbackWidth;
  const left = Math.max(
    16,
    Math.min(rect.right - width, window.innerWidth - width - 16),
  );
  panel.style.top = `${Math.round(rect.bottom + 8)}px`;
  panel.style.left = `${Math.round(left)}px`;
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

function currentView() {
  const params = new URLSearchParams(location.search);
  const groups = params.getAll('group');
  return {
    showHidden: parseQueryBool(params.get('hidden')) ?? defaultShowHidden(),
    showExcludes: canToggleExcludes()
      ? (parseQueryBool(params.get('excludes')) ?? defaultShowExcludes())
      : false,
    filterExt: parseQueryBool(params.get('filter')) ?? defaultFilterExt(),
    sort: parseSort(params.get('sort')) || defaultSort(),
    sortDir:
      parseSortDir(params.get('dir')) ||
      defaultSortDir(parseSort(params.get('sort')) || defaultSort()),
    filterGroups:
      groups.length > 0 ? [...new Set(groups)] : defaultFilterGroups(),
  };
}

function setQueryBool(params, key, value, defaultValue) {
  if (value === defaultValue) {
    params.delete(key);
  } else {
    params.set(key, value ? '1' : '0');
  }
}

function withView(urlLike, view = currentView()) {
  const url = new URL(urlLike, location.origin);
  setQueryBool(
    url.searchParams,
    'hidden',
    view.showHidden,
    defaultShowHidden(),
  );
  if (canToggleExcludes()) {
    setQueryBool(
      url.searchParams,
      'excludes',
      view.showExcludes,
      defaultShowExcludes(),
    );
  } else {
    url.searchParams.delete('excludes');
  }
  setQueryBool(url.searchParams, 'filter', view.filterExt, defaultFilterExt());
  if (view.sort === defaultSort()) {
    url.searchParams.delete('sort');
  } else {
    url.searchParams.set('sort', view.sort);
  }
  if (view.sortDir === defaultSortDir(view.sort)) {
    url.searchParams.delete('dir');
  } else {
    url.searchParams.set('dir', view.sortDir);
  }
  url.searchParams.delete('group');
  const groups = [...new Set(view.filterGroups)];
  const defaults = defaultFilterGroups();
  if (
    groups.length !== defaults.length ||
    groups.some((group, index) => group !== defaults[index])
  ) {
    for (const group of groups) {
      url.searchParams.append('group', group);
    }
  }
  return `${url.pathname}${url.search}${url.hash}`;
}

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

function icon(name) {
  return `<svg aria-hidden="true" height="16" width="16" class="ghrm-file-icon"><use href="#ghrm-icon-${name}"></use></svg>`;
}

function escapeHtml(value) {
  return value.replace(/[&<>"']/g, (ch) => {
    switch (ch) {
      case '&':
        return '&amp;';
      case '<':
        return '&lt;';
      case '>':
        return '&gt;';
      case '"':
        return '&quot;';
      default:
        return '&#39;';
    }
  });
}

function highlightMatch(value, query) {
  const lower = value.toLowerCase();
  const needle = query.toLowerCase();
  let start = 0;
  let out = '';

  for (;;) {
    const idx = lower.indexOf(needle, start);
    if (idx === -1) break;
    out += escapeHtml(value.slice(start, idx));
    out += `<strong class="ghrm-search-hit">${escapeHtml(value.slice(idx, idx + needle.length))}</strong>`;
    start = idx + needle.length;
  }

  return out + escapeHtml(value.slice(start));
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

function isHtmlFile(url) {
  try {
    const path = new URL(url, location.origin).pathname;
    return path.endsWith('.html') || path.endsWith('.htm');
  } catch {
    return false;
  }
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

function ensureNavTable(article) {
  const table = article.querySelector('.ghrm-nav-table');
  if (table) return table;

  const empty = article.querySelector('.ghrm-nav-empty');
  if (!empty) return null;
  const next = document.createElement('table');
  next.className = 'ghrm-nav-table';
  next.innerHTML = '<tbody></tbody>';
  empty.after(next);
  return next;
}

function renderSearchRows(tbody, results, query) {
  if (results.length === 0) {
    tbody.innerHTML =
      '<tr class="ghrm-search-empty"><td colspan="3">No matching paths.</td></tr>';
    return;
  }

  tbody.replaceChildren();
  for (const entry of results) {
    const id = entry.is_dir ? 'ghrm-search-dir-row' : 'ghrm-search-file-row';
    const tmpl = document.getElementById(id);
    const row = tmpl?.content.firstElementChild?.cloneNode(true);
    if (!row) continue;

    const link = row.querySelector('.ghrm-search-path');
    const date = row.querySelector('.ghrm-nav-date');
    link.href = withView(entry.href);
    link.innerHTML = highlightMatch(entry.display, query);
    if (entry.modified) {
      date.dataset.ts = String(entry.modified);
    }
    tbody.append(row);
  }
}

async function pathSearch(query, currentPath) {
  const view = currentView();
  const params = new URLSearchParams();
  params.set('q', query);
  if (currentPath) {
    params.set('path', currentPath);
  }
  params.set('hidden', view.showHidden ? '1' : '0');
  params.set('excludes', view.showExcludes ? '1' : '0');
  params.set('filter', view.filterExt ? '1' : '0');
  params.set('sort', view.sort);
  params.set('dir', view.sortDir);
  for (const group of view.filterGroups) {
    params.append('group', group);
  }
  const res = await fetch(`/_ghrm/path-search?${params}`).catch(() => null);
  if (!res || !res.ok) return { results: [], truncated: false, max_rows: 0 };
  return res
    .json()
    .catch(() => ({ results: [], truncated: false, max_rows: 0 }));
}

async function contentSearch(query) {
  const view = currentView();
  const params = new URLSearchParams();
  params.set('q', query);
  params.set('hidden', view.showHidden ? '1' : '0');
  params.set('excludes', view.showExcludes ? '1' : '0');
  params.set('filter', view.filterExt ? '1' : '0');
  params.set('sort', view.sort);
  params.set('dir', view.sortDir);
  for (const group of view.filterGroups) {
    params.append('group', group);
  }
  const res = await fetch(`/_ghrm/search?${params}`).catch(() => null);
  if (!res || !res.ok) return { results: [], truncated: false, max_rows: 0 };
  return res
    .json()
    .catch(() => ({ results: [], truncated: false, max_rows: 0 }));
}

const CONTENT_SNIPPET_MAX = 88;

function clampMatchWindow(text, ranges, max = CONTENT_SNIPPET_MAX) {
  if (!text) return { text: '', ranges: [], prefix: false, suffix: false };
  if (!ranges || ranges.length === 0) {
    if (text.length <= max) {
      return { text, ranges: [], prefix: false, suffix: false };
    }
    return {
      text: text.slice(0, max),
      ranges: [],
      prefix: false,
      suffix: true,
    };
  }

  const [firstStart, firstEnd] = ranges[0];
  let start = 0;
  if (firstEnd > max) {
    const center = Math.floor((firstStart + firstEnd) / 2);
    start = Math.max(0, center - Math.floor(max / 2));
  }
  const end = Math.min(text.length, start + max);
  if (end - start < max && end === text.length) {
    start = Math.max(0, end - max);
  }

  const clipped = [];
  for (const [rangeStart, rangeEnd] of ranges) {
    if (rangeEnd <= start || rangeStart >= end) continue;
    clipped.push([
      Math.max(rangeStart, start) - start,
      Math.min(rangeEnd, end) - start,
    ]);
  }

  return {
    text: text.slice(start, end),
    ranges: clipped,
    prefix: start > 0,
    suffix: end < text.length,
  };
}

function formatContentSnippet(text, ranges) {
  const clipped = clampMatchWindow(text, ranges);
  let html = highlightRanges(clipped.text, clipped.ranges);
  if (clipped.prefix) html = `... ${html}`;
  if (clipped.suffix) html = `${html} ...`;
  return html;
}

function renderContentRows(tbody, results, truncated, maxRows) {
  if (results.length === 0) {
    tbody.innerHTML =
      '<tr class="ghrm-search-empty"><td colspan="3">No matches found.</td></tr>';
    return;
  }

  tbody.replaceChildren();
  const tmpl = document.getElementById('ghrm-content-search-row');
  for (const match of results) {
    const row = tmpl?.content.firstElementChild?.cloneNode(true);
    if (!row) continue;

    const link = row.querySelector('.ghrm-content-path');
    const textEl = row.querySelector('.ghrm-content-text');

    link.href = withView(`/${match.path}`);
    link.innerHTML =
      `<strong>${escapeHtml(match.path)}</strong>` +
      `<span class="ghrm-content-line">:${match.line}</span>`;
    textEl.innerHTML = formatContentSnippet(match.text, match.ranges);
    tbody.append(row);
  }

  const note = document.createElement('tr');
  note.className = truncated ? 'ghrm-search-truncated' : 'ghrm-search-summary';
  note.innerHTML =
    '<td class="ghrm-nav-icon"></td>' +
    `<td class="ghrm-search-summary-cell" colspan="2">` +
    `<span>${truncated ? 'Results truncated' : ''}</span>` +
    `<span class="ghrm-search-summary-count">${results.length}/${maxRows}</span>` +
    '</td>';
  tbody.append(note);
}

function highlightRanges(text, ranges) {
  if (!ranges || ranges.length === 0) {
    return escapeHtml(text);
  }
  let result = '';
  let pos = 0;
  for (const [start, end] of ranges) {
    if (start > pos) {
      result += escapeHtml(text.slice(pos, start));
    }
    result += `<mark>${escapeHtml(text.slice(start, end))}</mark>`;
    pos = end;
  }
  if (pos < text.length) {
    result += escapeHtml(text.slice(pos));
  }
  return result;
}

function setupPathSearch() {
  const article = document.querySelector('article[data-explorer]');
  const search = document.getElementById('ghrm-path-search');
  const input = document.getElementById('ghrm-path-search-input');
  const button = document.getElementById('ghrm-path-search-toggle');
  const modeBtn = document.getElementById('ghrm-search-mode');
  const status = document.getElementById('ghrm-path-search-status');
  const table = article ? ensureNavTable(article) : null;
  const tbody = table?.querySelector('tbody');
  if (!search || !input || !button || !status) return;

  search.hidden = !article;
  search.classList.remove('is-open');
  search.dataset.mode = searchMode;
  input.value = '';
  input.placeholder =
    searchMode === 'content' ? 'Search content' : 'Search paths';
  input.tabIndex = -1;
  input.oninput = null;
  input.onkeydown = null;
  button.onclick = null;
  if (modeBtn) modeBtn.onclick = null;
  button.setAttribute('aria-expanded', 'false');
  status.textContent = '';
  if (!article || !table || !tbody) return;

  const empty = article.querySelector('.ghrm-nav-empty');
  const originalRows = tbody.innerHTML;
  const currentPath = article.dataset.currentPath ?? '';
  let searchSeq = 0;
  if (!originalRows.trim()) {
    table.hidden = true;
  }

  const resetSearch = () => {
    tbody.innerHTML = originalRows;
    table.hidden = !originalRows.trim();
    if (empty) empty.hidden = false;
    status.textContent = '';
    populateDates();
    setupNavExternalLinks();
  };

  const updateMode = () => {
    search.dataset.mode = searchMode;
    input.placeholder =
      searchMode === 'content' ? 'Search content' : 'Search paths';
    if (modeBtn) {
      const label =
        searchMode === 'content'
          ? 'Switch to path search'
          : 'Switch to content search';
      modeBtn.title = label;
      modeBtn.setAttribute('aria-label', label);
    }
  };

  if (modeBtn) {
    modeBtn.onclick = () => {
      const query = input.value.trim();
      searchMode = searchMode === 'path' ? 'content' : 'path';
      updateMode();
      if (!query) {
        searchSeq += 1;
        resetSearch();
      } else {
        input.oninput?.();
      }
      input.focus();
    };
  }
  updateMode();

  button.onclick = () => {
    const open = !search.classList.contains('is-open');
    search.classList.toggle('is-open', open);
    button.setAttribute('aria-expanded', open ? 'true' : 'false');
    input.tabIndex = open ? 0 : -1;
    if (open) {
      input.focus();
    } else {
      input.value = '';
      searchSeq += 1;
      resetSearch();
    }
  };

  input.oninput = async () => {
    searchSeq += 1;
    const seq = searchSeq;
    const query = input.value.trim();
    if (!query) {
      resetSearch();
      return;
    }

    if (searchMode === 'content') {
      status.textContent = 'Searching...';
      const resp = await contentSearch(query);
      if (seq !== searchSeq) return;
      if (empty) empty.hidden = true;
      table.hidden = false;
      renderContentRows(tbody, resp.results, resp.truncated, resp.max_rows);
      const count = resp.results.length;
      const suffix = resp.truncated ? '+' : '';
      status.textContent =
        count === 1 ? '1 match' : `${count}${suffix} matches`;
    } else {
      const resp = await pathSearch(query, currentPath);
      if (seq !== searchSeq) return;
      const results = resp.results ?? [];
      if (empty) empty.hidden = true;
      table.hidden = false;
      renderSearchRows(tbody, results, query);
      const suffix = resp.truncated ? '+' : '';
      status.textContent =
        results.length === 1
          ? `1${suffix} path`
          : `${results.length}${suffix} paths`;
      populateDates();
      setupNavExternalLinks();
    }
  };

  input.onkeydown = (e) => {
    if (e.key !== 'Escape') return;
    search.classList.remove('is-open');
    button.setAttribute('aria-expanded', 'false');
    input.tabIndex = -1;
    input.value = '';
    searchSeq += 1;
    resetSearch();
    button.focus();
  };
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
    navigate(withView(a.href));
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
  setupPathSearch();
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
  positionFloatingPanel(panel, btn, 248);
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
  setupPathSearch();
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

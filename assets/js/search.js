import { escapeHtml } from './dom.js';
import { beginActivity, endActivity } from './status.js';
import { columnKeys, currentView, withView } from './view.js';

const SEARCH_COLUMN_KEYS = ['date'];

let searchMode = 'path';
let searchOpen = false;
let searchQuery = '';
let refreshSearch = null;
let closeDirtySearch = null;
let searchDirty = false;
let searchView = null;

export function hasActiveSearch() {
  return searchOpen && searchQuery.trim().length > 0 && Boolean(refreshSearch);
}

export function refreshActiveSearch(view = null) {
  if (!hasActiveSearch()) return false;
  searchDirty = true;
  searchView = view;
  refreshSearch();
  return true;
}

export function setSearchCloseHandler(handler) {
  closeDirtySearch = handler;
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

function renderColumnCell(cell) {
  const td = document.createElement('td');
  td.className = cell.class || '';
  td.dataset.columnKey = cell.key || '';
  td.hidden = Boolean(cell.hidden);
  if (cell.timestamp !== null && cell.timestamp !== undefined) {
    td.dataset.ts = String(cell.timestamp);
  }
  if (cell.text) {
    if (cell.text_class) {
      const span = document.createElement('span');
      span.className = cell.text_class;
      span.textContent = cell.text;
      td.append(span);
    } else {
      td.textContent = cell.text;
    }
  }
  return td;
}

function pathSearchColumnKeys(view) {
  return new Set(view.columns);
}

function contentSearchColumnKeys() {
  return new Set(SEARCH_COLUMN_KEYS);
}

function activeSearchColumnKeys(view) {
  return searchMode === 'content'
    ? contentSearchColumnKeys()
    : pathSearchColumnKeys(view);
}

function searchColspan(keys) {
  return keys.size + 2;
}

function fullColspan() {
  return columnKeys().length + 2;
}

function contentColspan() {
  const dateIndex = columnKeys().indexOf('date');
  if (dateIndex === -1) return fullColspan() - 1;
  return dateIndex + 1;
}

function applySearchColumns(article, keys) {
  for (const cell of article.querySelectorAll('[data-column-key]')) {
    cell.hidden = !keys.has(cell.dataset.columnKey);
  }
}

function renderSearchRows(article, tbody, results, query, view) {
  const keys = pathSearchColumnKeys(view);
  if (results.length === 0) {
    tbody.innerHTML = `<tr class="ghrm-search-empty"><td colspan="${searchColspan(keys)}">No matching paths.</td></tr>`;
    applySearchColumns(article, keys);
    return;
  }

  tbody.replaceChildren();
  for (const entry of results) {
    const id = entry.is_dir ? 'ghrm-search-dir-row' : 'ghrm-search-file-row';
    const tmpl = document.getElementById(id);
    const row = tmpl?.content.firstElementChild?.cloneNode(true);
    if (!row) continue;

    const link = row.querySelector('.ghrm-search-path');
    link.href = withView(entry.href, view);
    link.innerHTML = highlightMatch(entry.display, query);
    for (const cell of row.querySelectorAll('[data-column-key]')) {
      cell.remove();
    }
    for (const cell of entry.cells || []) {
      row.append(renderColumnCell(cell));
    }
    tbody.append(row);
  }
  applySearchColumns(article, keys);
}

function buildSearchParams(query, extraParams = {}, view = currentView()) {
  const params = new URLSearchParams();
  params.set('q', query);
  for (const [k, v] of Object.entries(extraParams)) {
    params.set(k, v);
  }
  params.set('hidden', view.showHidden ? '1' : '0');
  params.set('excludes', view.showExcludes ? '1' : '0');
  params.set('ignore', view.useIgnore ? '1' : '0');
  params.set('filter', view.filterExt ? '1' : '0');
  params.set('sort', view.sort);
  params.set('dir', view.sortDir);
  for (const key of columnKeys()) {
    params.set(key, view.columns.has(key) ? '1' : '0');
  }
  for (const group of view.filterGroups) {
    params.append('group', group);
  }
  return params;
}

async function pathSearch(query, currentPath, view) {
  const extra = currentPath ? { path: currentPath } : {};
  const params = buildSearchParams(query, extra, view);
  const res = await fetch(`/_ghrm/path-search?${params}`).catch(() => null);
  if (!res || !res.ok)
    return { results: [], truncated: false, max_rows: 0, pending: false };
  return res.json().catch(() => ({
    results: [],
    truncated: false,
    max_rows: 0,
    pending: false,
  }));
}

async function contentSearch(query, view) {
  const params = buildSearchParams(query, {}, view);
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

function formatContentSnippet(text, ranges) {
  const clipped = clampMatchWindow(text, ranges);
  let html = highlightRanges(clipped.text, clipped.ranges);
  if (clipped.prefix) html = `... ${html}`;
  if (clipped.suffix) html = `${html} ...`;
  return html;
}

function renderContentRows(article, tbody, results, truncated, maxRows, view) {
  const keys = contentSearchColumnKeys();
  if (results.length === 0) {
    tbody.innerHTML = `<tr class="ghrm-search-empty"><td colspan="${fullColspan()}">No matches found.</td></tr>`;
    applySearchColumns(article, keys);
    return;
  }

  tbody.replaceChildren();
  const tmpl = document.getElementById('ghrm-content-search-row');
  for (const match of results) {
    const row = tmpl?.content.firstElementChild?.cloneNode(true);
    if (!row) continue;

    const link = row.querySelector('.ghrm-content-path');
    const textEl = row.querySelector('.ghrm-content-text');
    const cell = row.querySelector('.ghrm-content-cell');
    if (cell) {
      cell.colSpan = contentColspan();
    }

    link.href = withView(`/${match.path}`, view);
    link.innerHTML =
      `<strong>${escapeHtml(match.path)}</strong>` +
      `<span class="ghrm-content-line">:${match.line}</span>`;
    textEl.innerHTML = formatContentSnippet(match.text, match.ranges);
    row.append(
      renderColumnCell({
        key: 'date',
        class: 'ghrm-nav-meta ghrm-nav-meta-time ghrm-nav-edge-meta',
        timestamp: match.modified,
      }),
    );
    tbody.append(row);
  }

  const note = document.createElement('tr');
  note.className = truncated ? 'ghrm-search-truncated' : 'ghrm-search-summary';
  note.innerHTML =
    '<td class="ghrm-nav-icon"></td>' +
    `<td class="ghrm-search-summary-cell" colspan="${fullColspan() - 1}">` +
    `<span>${truncated ? 'Results truncated' : ''}</span>` +
    `<span class="ghrm-search-summary-count">${results.length}/${maxRows}</span>` +
    '</td>';
  tbody.append(note);
  applySearchColumns(article, keys);
}

export function setupPathSearch({
  populateDates,
  setupNavExternalLinks,
  syncColumnControls,
}) {
  const article = document.querySelector('article[data-explorer]');
  const search = document.getElementById('ghrm-path-search');
  const input = document.getElementById('ghrm-path-search-input');
  const button = document.getElementById('ghrm-path-search-toggle');
  const modeBtn = document.getElementById('ghrm-search-mode');
  const status = document.getElementById('ghrm-path-search-status');
  const table = article ? ensureNavTable(article) : null;
  const tbody = table?.querySelector('tbody');
  if (!search || !input || !button || !status) return;

  const restoredOpen = searchOpen && Boolean(article);
  searchView = null;
  search.hidden = !article;
  search.classList.toggle('is-open', restoredOpen);
  search.dataset.mode = searchMode;
  input.value = restoredOpen ? searchQuery : '';
  input.placeholder =
    searchMode === 'content' ? 'Search content' : 'Search paths';
  input.tabIndex = restoredOpen ? 0 : -1;
  input.oninput = null;
  input.onkeydown = null;
  button.onclick = null;
  if (modeBtn) modeBtn.onclick = null;
  button.setAttribute('aria-expanded', restoredOpen ? 'true' : 'false');
  status.textContent = '';
  refreshSearch = null;
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
    syncColumnControls();
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

  const closeSearch = () => {
    search.classList.remove('is-open');
    searchOpen = false;
    searchView = null;
    button.setAttribute('aria-expanded', 'false');
    input.tabIndex = -1;
    input.value = '';
    searchQuery = '';
    searchSeq += 1;
    if (searchDirty && closeDirtySearch) {
      searchDirty = false;
      closeDirtySearch();
    } else {
      resetSearch();
    }
  };

  if (modeBtn) {
    modeBtn.onclick = () => {
      searchQuery = input.value;
      const query = searchQuery.trim();
      searchMode = searchMode === 'path' ? 'content' : 'path';
      updateMode();
      if (!query) {
        searchSeq += 1;
        resetSearch();
      } else {
        doSearch();
      }
      input.focus();
    };
  }
  updateMode();

  button.onclick = () => {
    const open = !search.classList.contains('is-open');
    searchOpen = open;
    search.classList.toggle('is-open', open);
    button.setAttribute('aria-expanded', open ? 'true' : 'false');
    input.tabIndex = open ? 0 : -1;
    if (open) {
      input.focus();
    } else {
      closeSearch();
    }
  };

  const doSearch = async () => {
    searchSeq += 1;
    const seq = searchSeq;
    searchQuery = input.value;
    const query = searchQuery.trim();
    const view = searchView || currentView();
    if (!query) {
      if (searchDirty && closeDirtySearch) {
        searchDirty = false;
        closeDirtySearch();
      } else {
        resetSearch();
      }
      return;
    }
    beginActivity();
    try {
      applySearchColumns(article, activeSearchColumnKeys(view));

      if (searchMode === 'content') {
        status.textContent = 'Searching...';
        const resp = await contentSearch(query, view);
        if (seq !== searchSeq) return;
        if (empty) empty.hidden = true;
        table.hidden = false;
        renderContentRows(
          article,
          tbody,
          resp.results,
          resp.truncated,
          resp.max_rows,
          view,
        );
        const count = resp.results.length;
        const suffix = resp.truncated ? '+' : '';
        status.textContent =
          count === 1 ? '1 match' : `${count}${suffix} matches`;
        populateDates();
      } else {
        const resp = await pathSearch(query, currentPath, view);
        if (seq !== searchSeq) return;
        if (resp.pending) {
          status.textContent = 'Indexing paths...';
          return;
        }
        const results = resp.results ?? [];
        if (empty) empty.hidden = true;
        table.hidden = false;
        renderSearchRows(article, tbody, results, query, view);
        const suffix = resp.truncated ? '+' : '';
        status.textContent =
          results.length === 1
            ? `1${suffix} path`
            : `${results.length}${suffix} paths`;
        populateDates();
        setupNavExternalLinks();
      }
    } finally {
      endActivity();
    }
  };

  input.oninput = doSearch;

  input.onkeydown = (e) => {
    if (e.key !== 'Escape') return;
    closeSearch();
    button.focus();
  };

  refreshSearch = doSearch;
  if (restoredOpen && searchQuery.trim()) {
    doSearch();
  }
}

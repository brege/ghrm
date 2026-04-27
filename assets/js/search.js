import { escapeHtml } from './dom.js';
import { currentView, withView } from './view.js';

let searchMode = 'path';

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

export function setupPathSearch({ populateDates, setupNavExternalLinks }) {
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

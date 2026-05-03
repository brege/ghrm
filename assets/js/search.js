import { beginActivity, endActivity } from './status.js';

let searchMode = 'path';
let searchOpen = false;
let searchQuery = '';
let refreshSearch = null;
let closeDirtySearch = null;
let searchDirty = false;

export function hasActiveSearch() {
  return searchOpen && searchQuery.trim().length > 0 && Boolean(refreshSearch);
}

export function refreshActiveSearch() {
  if (!hasActiveSearch()) return false;
  searchDirty = true;
  refreshSearch();
  return true;
}

export function setSearchCloseHandler(handler) {
  closeDirtySearch = handler;
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

function buildSearchParams(query, extraParams = {}) {
  const params = new URLSearchParams(location.search);
  params.set('q', query);
  for (const [key, value] of Object.entries(extraParams)) {
    if (value) {
      params.set(key, value);
    } else {
      params.delete(key);
    }
  }
  return params;
}

async function searchFragment(endpoint, params) {
  const res = await fetch(`${endpoint}?${params}`, {
    headers: {
      Accept: 'text/html',
      'HX-Request': 'true',
    },
  }).catch(() => null);
  if (!res || !res.ok) {
    return {
      html: '',
      count: 0,
      truncated: false,
      pending: false,
      maxRows: 0,
    };
  }
  return {
    html: await res.text(),
    count: parseInt(res.headers.get('X-Ghrm-Search-Count') || '0', 10),
    truncated: res.headers.get('X-Ghrm-Search-Truncated') === '1',
    pending: res.headers.get('X-Ghrm-Search-Pending') === '1',
    maxRows: parseInt(res.headers.get('X-Ghrm-Search-Max-Rows') || '0', 10),
  };
}

function pathSearch(query, currentPath) {
  const params = buildSearchParams(query, { path: currentPath });
  return searchFragment('/_ghrm/path-search', params);
}

function contentSearch(query) {
  const params = buildSearchParams(query);
  return searchFragment('/_ghrm/search', params);
}

function setRows(tbody, resp) {
  tbody.innerHTML = resp.html;
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
      if (searchMode === 'content') {
        status.textContent = 'Searching...';
        const resp = await contentSearch(query);
        if (seq !== searchSeq) return;
        if (empty) empty.hidden = true;
        table.hidden = false;
        setRows(tbody, resp);
        const suffix = resp.truncated ? '+' : '';
        status.textContent =
          resp.count === 1 ? '1 match' : `${resp.count}${suffix} matches`;
        populateDates();
      } else {
        const resp = await pathSearch(query, currentPath);
        if (seq !== searchSeq) return;
        if (empty) empty.hidden = true;
        table.hidden = false;
        setRows(tbody, resp);
        if (resp.pending) {
          status.textContent = 'Indexing paths...';
          return;
        }
        const suffix = resp.truncated ? '+' : '';
        status.textContent =
          resp.count === 1 ? `1${suffix} path` : `${resp.count}${suffix} paths`;
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

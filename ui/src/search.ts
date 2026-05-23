import { qsel, qselFrom } from './dom';
import { beginActivity, endActivity } from './status';

export interface SearchSetupOptions {
  populateDates: () => void;
  setupNavExternalLinks: () => void;
  syncColumnControls: () => void;
}

interface SearchFragmentResponse {
  html: string;
  count: number;
  truncated: boolean;
  pending: boolean;
  maxRows: number;
}

type SearchMode = 'path' | 'content';

let searchMode: SearchMode = 'path';
let searchOpen = false;
let searchQuery = '';
let refreshSearch: (() => void) | null = null;
let closeDirtySearch: (() => void) | null = null;
let searchDirty = false;

export function hasActiveSearch(): boolean {
  return searchOpen && searchQuery.trim().length > 0 && Boolean(refreshSearch);
}

export function refreshActiveSearch(): boolean {
  const refresh = refreshSearch;
  if (!hasActiveSearch() || !refresh) return false;
  searchDirty = true;
  refresh();
  return true;
}

export function setSearchCloseHandler(handler: () => void): void {
  closeDirtySearch = handler;
}

function ensureNavTable(article: Element): HTMLTableElement | null {
  const table = article.querySelector('.ghrm-nav-table');
  if (table instanceof HTMLTableElement) return table;

  const empty = article.querySelector('.ghrm-nav-empty');
  if (!empty) return null;
  const next = document.createElement('table');
  next.className = 'ghrm-nav-table';
  next.innerHTML = '<tbody></tbody>';
  empty.after(next);
  return next;
}

function buildSearchParams(
  query: string,
  extraParams: Record<string, string | undefined> = {},
): URLSearchParams {
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

async function searchFragment(
  endpoint: string,
  params: URLSearchParams,
): Promise<SearchFragmentResponse> {
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

function fetchPathResults(
  query: string,
  currentPath: string,
): Promise<SearchFragmentResponse> {
  const params = buildSearchParams(query, { path: currentPath });
  return searchFragment('/_ghrm/path-search', params);
}

function fetchContentResults(query: string): Promise<SearchFragmentResponse> {
  const params = buildSearchParams(query);
  return searchFragment('/_ghrm/search', params);
}

function setRows(
  tbody: HTMLTableSectionElement,
  resp: SearchFragmentResponse,
): void {
  tbody.innerHTML = resp.html;
}

export function setupPathSearch(options: SearchSetupOptions): void {
  const { populateDates, setupNavExternalLinks, syncColumnControls } = options;
  const article = qsel('article[data-explorer]');
  const search = qsel('#ghrm-path-search');
  const inputEl = document.querySelector('#ghrm-path-search-input');
  const input = inputEl instanceof HTMLInputElement ? inputEl : null;
  const button = document.getElementById('ghrm-path-search-toggle');
  const modeBtn = document.getElementById('ghrm-search-mode');
  const status = document.getElementById('ghrm-path-search-status');
  const table = article ? ensureNavTable(article) : null;
  const tbody = table?.querySelector('tbody') as HTMLTableSectionElement | null;
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

  const empty = qselFrom(article, '.ghrm-nav-empty');
  const originalRows = tbody.innerHTML;
  const currentPath =
    (article instanceof HTMLElement
      ? article.dataset.currentPath
      : undefined) ?? '';
  let searchSeq = 0;
  if (!originalRows.trim()) {
    table.hidden = true;
  }

  const resetSearch = (): void => {
    tbody.innerHTML = originalRows;
    syncColumnControls();
    table.hidden = !originalRows.trim();
    if (empty) empty.hidden = false;
    status.textContent = '';
    populateDates();
    setupNavExternalLinks();
  };

  const updateMode = (): void => {
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

  const closeSearch = (): void => {
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
    modeBtn.onclick = (): void => {
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

  button.onclick = (): void => {
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

  const doSearch = async (): Promise<void> => {
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
        const resp = await fetchContentResults(query);
        if (seq !== searchSeq) return;
        if (empty) empty.hidden = true;
        table.hidden = false;
        setRows(tbody, resp);
        const suffix = resp.truncated ? '+' : '';
        status.textContent =
          resp.count === 1 ? '1 match' : `${resp.count}${suffix} matches`;
        populateDates();
      } else {
        const resp = await fetchPathResults(query, currentPath);
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

  input.onkeydown = (e): void => {
    if (e.key !== 'Escape') return;
    closeSearch();
    button.focus();
  };

  refreshSearch = doSearch;
  if (restoredOpen && searchQuery.trim()) {
    doSearch();
  }
}

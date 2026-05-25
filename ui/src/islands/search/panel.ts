import { LitElement } from 'lit';
import { qsel, qselFrom } from '../../dom';
import {
  populateDates,
  setupNavExternalLinks,
  syncColumnControls,
} from '../../explorer';
import { beginActivity, endActivity } from '../../status';

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

export function resetSearchState(): void {
  searchMode = 'path';
  searchOpen = false;
  searchQuery = '';
  refreshSearch = null;
  closeDirtySearch = null;
  searchDirty = false;
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

export class GhrmSearchPanel extends LitElement {
  private searchSeq = 0;
  private originalRows = '';
  private currentPath = '';

  private search: HTMLElement | null = null;
  private input: HTMLInputElement | null = null;
  private button: HTMLElement | null = null;
  private modeBtn: HTMLElement | null = null;
  private status: HTMLElement | null = null;
  private table: HTMLTableElement | null = null;
  private tbody: HTMLTableSectionElement | null = null;
  private empty: HTMLElement | null = null;

  private boundContentReady: (() => void) | null = null;

  protected createRenderRoot(): HTMLElement {
    return this;
  }

  connectedCallback(): void {
    super.connectedCallback();
    this.setupSearch();
    this.boundContentReady = () => this.handleContentReady();
    document.addEventListener('ghrm:contentready', this.boundContentReady);
  }

  disconnectedCallback(): void {
    super.disconnectedCallback();
    this.clearHandlers();
    if (this.boundContentReady) {
      document.removeEventListener('ghrm:contentready', this.boundContentReady);
      this.boundContentReady = null;
    }
    refreshSearch = null;
  }

  private handleContentReady(): void {
    this.searchSeq += 1;
    this.clearHandlers();
    this.setupSearch();
  }

  private clearHandlers(): void {
    if (this.input) {
      this.input.oninput = null;
      this.input.onkeydown = null;
    }
    if (this.button) this.button.onclick = null;
    if (this.modeBtn) this.modeBtn.onclick = null;
  }

  private setupSearch(): void {
    const article = qsel('article[data-explorer]');
    this.search = qsel('#ghrm-path-search');
    const inputEl = document.querySelector('#ghrm-path-search-input');
    this.input = inputEl instanceof HTMLInputElement ? inputEl : null;
    this.button = document.getElementById('ghrm-path-search-toggle');
    this.modeBtn = document.getElementById('ghrm-search-mode');
    this.status = document.getElementById('ghrm-path-search-status');
    this.table = article ? ensureNavTable(article) : null;
    this.tbody = this.table?.querySelector('tbody') ?? null;

    if (!this.search || !this.input || !this.button || !this.status) return;

    const restoredOpen = searchOpen && Boolean(article);
    this.search.hidden = !article;
    this.search.classList.toggle('is-open', restoredOpen);
    this.search.dataset.mode = searchMode;
    this.input.value = restoredOpen ? searchQuery : '';
    this.input.placeholder =
      searchMode === 'content' ? 'Search content' : 'Search paths';
    this.input.tabIndex = restoredOpen ? 0 : -1;
    this.clearHandlers();
    this.button.setAttribute('aria-expanded', restoredOpen ? 'true' : 'false');
    this.status.textContent = '';
    refreshSearch = null;

    if (!article || !this.table || !this.tbody) return;

    this.empty = qselFrom(article, '.ghrm-nav-empty');
    this.originalRows = this.tbody.innerHTML;
    this.currentPath =
      (article instanceof HTMLElement
        ? article.dataset.currentPath
        : undefined) ?? '';

    if (!this.originalRows.trim()) {
      this.table.hidden = true;
    }

    this.updateMode();
    this.bindHandlers();

    refreshSearch = () => this.doSearch();
    if (restoredOpen && searchQuery.trim()) {
      this.doSearch();
    }
  }

  private bindHandlers(): void {
    if (this.modeBtn) {
      this.modeBtn.onclick = () => this.handleModeSwitch();
    }

    if (this.button) {
      this.button.onclick = () => this.handleToggle();
    }

    if (this.input) {
      this.input.oninput = () => this.doSearch();
      this.input.onkeydown = (e) => this.handleKeydown(e);
    }
  }

  private handleModeSwitch(): void {
    if (!this.input) return;
    searchQuery = this.input.value;
    const query = searchQuery.trim();
    searchMode = searchMode === 'path' ? 'content' : 'path';
    this.updateMode();
    if (!query) {
      this.searchSeq += 1;
      this.resetSearch();
    } else {
      this.doSearch();
    }
    this.input.focus();
  }

  private handleToggle(): void {
    if (!this.search || !this.input || !this.button) return;
    const open = !this.search.classList.contains('is-open');
    searchOpen = open;
    this.search.classList.toggle('is-open', open);
    this.button.setAttribute('aria-expanded', open ? 'true' : 'false');
    this.input.tabIndex = open ? 0 : -1;
    if (open) {
      this.input.focus();
    } else {
      this.closeSearch();
    }
  }

  private handleKeydown(e: KeyboardEvent): void {
    if (e.key !== 'Escape') return;
    this.closeSearch();
    this.button?.focus();
  }

  private updateMode(): void {
    if (!this.search || !this.input) return;
    this.search.dataset.mode = searchMode;
    this.input.placeholder =
      searchMode === 'content' ? 'Search content' : 'Search paths';
    if (this.modeBtn) {
      const label =
        searchMode === 'content'
          ? 'Switch to path search'
          : 'Switch to content search';
      this.modeBtn.title = label;
      this.modeBtn.setAttribute('aria-label', label);
    }
  }

  private resetSearch(): void {
    if (!this.tbody || !this.table || !this.status) return;
    this.tbody.innerHTML = this.originalRows;
    syncColumnControls();
    this.table.hidden = !this.originalRows.trim();
    if (this.empty) this.empty.hidden = false;
    this.status.textContent = '';
    populateDates();
    setupNavExternalLinks();
  }

  private closeSearch(): void {
    if (!this.search || !this.input || !this.button) return;
    this.search.classList.remove('is-open');
    searchOpen = false;
    this.button.setAttribute('aria-expanded', 'false');
    this.input.tabIndex = -1;
    this.input.value = '';
    searchQuery = '';
    this.searchSeq += 1;
    if (searchDirty && closeDirtySearch) {
      searchDirty = false;
      closeDirtySearch();
    } else {
      this.resetSearch();
    }
  }

  private async doSearch(): Promise<void> {
    if (!this.input || !this.tbody || !this.table || !this.status) return;

    this.searchSeq += 1;
    const seq = this.searchSeq;
    searchQuery = this.input.value;
    const query = searchQuery.trim();

    if (!query) {
      if (searchDirty && closeDirtySearch) {
        searchDirty = false;
        closeDirtySearch();
      } else {
        this.resetSearch();
      }
      return;
    }

    beginActivity();
    try {
      if (searchMode === 'content') {
        this.status.textContent = 'Searching...';
        const resp = await fetchContentResults(query);
        if (seq !== this.searchSeq) return;
        if (this.empty) this.empty.hidden = true;
        this.table.hidden = false;
        this.tbody.innerHTML = resp.html;
        const suffix = resp.truncated ? '+' : '';
        this.status.textContent =
          resp.count === 1 ? '1 match' : `${resp.count}${suffix} matches`;
        populateDates();
      } else {
        const resp = await fetchPathResults(query, this.currentPath);
        if (seq !== this.searchSeq) return;
        if (this.empty) this.empty.hidden = true;
        this.table.hidden = false;
        this.tbody.innerHTML = resp.html;
        if (resp.pending) {
          this.status.textContent = 'Indexing paths...';
          return;
        }
        const suffix = resp.truncated ? '+' : '';
        this.status.textContent =
          resp.count === 1 ? `1${suffix} path` : `${resp.count}${suffix} paths`;
        populateDates();
        setupNavExternalLinks();
      }
    } finally {
      endActivity();
    }
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'ghrm-search-panel': GhrmSearchPanel;
  }
}

customElements.define('ghrm-search-panel', GhrmSearchPanel);

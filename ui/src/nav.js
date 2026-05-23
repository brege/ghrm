import { scrollToHash } from './dom';
import {
  populateDates,
  setupNavExternalLinks,
  setupViewMenu,
  syncColumnControls,
} from './explorer';
import { setupFileViews } from './file';
import { applyDocChromePref, setupDocChromeToggle } from './prefs';
import { setSearchCloseHandler, setupPathSearch } from './search';
import { beginActivity, endActivity, syncServerStatus } from './status';
import { buildToc, setupToc } from './toc';

let pendingSamePathSwap = false;

export function setupInitialContent() {
  setupFileViews();
  setupSearch();
  setupViewMenu();
  setupDocChromeToggle();
  populateDates();
  setupToc();
  setupNavExternalLinks();
}

export function setupHtmxNav() {
  document.body.addEventListener('htmx:beforeBoost', (e) => {
    const link = e.detail.elt?.closest?.('a');
    if (link && !shouldBoostLink(link)) {
      e.preventDefault();
    }
  });

  document.body.addEventListener('htmx:afterSwap', (e) => {
    if (e.detail.target?.matches('article.markdown-body')) {
      afterContentSwap(e.detail.xhr);
    }
  });

  document.body.addEventListener('htmx:historyRestore', () => {
    refreshContent({ resetScroll: false });
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

function setupSearch() {
  setSearchCloseHandler(() => {
    const target = `${location.pathname}${location.search}${location.hash}`;
    location.assign(target);
  });
  setupPathSearch({ populateDates, setupNavExternalLinks, syncColumnControls });
}

function refreshContent({ resetScroll }) {
  syncServerStatus();
  setupFileViews();
  setupSearch();
  setupNavExternalLinks();
  setupViewMenu();
  syncColumnControls();
  applyDocChromePref();
  populateDates();
  buildToc();
  if (resetScroll) {
    const hash = location.hash;
    if (hash) {
      scrollToHash(hash);
    } else if (!pendingSamePathSwap) {
      window.scrollTo(0, 0);
    }
  }
  pendingSamePathSwap = false;
  document.dispatchEvent(new CustomEvent('ghrm:contentready'));
}

function afterContentSwap(xhr) {
  const title = xhr.getResponseHeader('HX-Title');
  if (title !== null) {
    document.title = decodeURIComponent(title);
  }
  refreshContent({ resetScroll: true });
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

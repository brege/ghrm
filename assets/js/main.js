import { scrollToHash } from './dom.js';
import {
  populateDates,
  setupNavExternalLinks,
  setupViewMenu,
  syncColumnControls,
} from './explorer.js';
import { setupFileViews } from './file.js';
import {
  applyDocChromePref,
  setupDocChromeToggle,
  setupThemeToggle,
} from './prefs.js';
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

let pendingSamePathSwap = false;

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

function afterContentSwap(xhr) {
  const title = xhr.getResponseHeader('HX-Title');
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
      afterContentSwap(e.detail.xhr);
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

import { scrollToHash } from './dom';
import { runRefresh } from './runtime';
import { beginActivity, endActivity, syncServerStatus } from './status';

let pendingSamePathSwap = false;

export function setupHtmxNav(): void {
  document.body.addEventListener('htmx:beforeBoost', (e) => {
    const link = e.detail.elt?.closest?.('a');
    if (link && !shouldBoostLink(link as HTMLAnchorElement)) {
      e.preventDefault();
    }
  });

  document.body.addEventListener('htmx:afterSwap', (e) => {
    if (e.detail.target?.matches('article.markdown-body')) {
      afterContentSwap(e.detail.xhr as XMLHttpRequest);
    }
  });

  document.body.addEventListener('htmx:historyRestore', () => {
    refreshContent({ resetScroll: false });
  });

  document.body.addEventListener('htmx:beforeRequest', (e) => {
    if (e.detail.target?.matches('article.markdown-body')) {
      const link = e.detail.elt?.closest?.('a');
      pendingSamePathSwap = link
        ? new URL((link as HTMLAnchorElement).href, location.origin)
            .pathname === location.pathname
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

function refreshContent({ resetScroll }: { resetScroll: boolean }): void {
  runRefresh();
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

function afterContentSwap(xhr: XMLHttpRequest): void {
  const title = xhr.getResponseHeader('HX-Title');
  if (title !== null) {
    document.title = decodeURIComponent(title);
  }
  refreshContent({ resetScroll: true });
}

function shouldBoostLink(a: HTMLAnchorElement): boolean {
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

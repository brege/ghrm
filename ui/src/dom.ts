export function qsel(selector: string): HTMLElement | null {
  const el = document.querySelector(selector);
  return el instanceof HTMLElement ? el : null;
}

export function qselFrom(
  root: ParentNode,
  selector: string,
): HTMLElement | null {
  const el = root.querySelector(selector);
  return el instanceof HTMLElement ? el : null;
}

export function qselAll(selector: string): HTMLElement[] {
  return [...document.querySelectorAll(selector)].filter(
    (el): el is HTMLElement => el instanceof HTMLElement,
  );
}

export function qselAllFrom(root: ParentNode, selector: string): HTMLElement[] {
  return [...root.querySelectorAll(selector)].filter(
    (el): el is HTMLElement => el instanceof HTMLElement,
  );
}

export function asElement(target: EventTarget | null): Element | null {
  return target instanceof Element ? target : null;
}

export function icon(name: string, cls = 'ghrm-file-icon'): string {
  return `<svg aria-hidden="true" height="16" width="16" class="${cls}"><use href="/_ghrm/assets/js/icons.svg#ghrm-icon-${name}"></use></svg>`;
}

export function escapeHtml(value: string): string {
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

export function visiblePane(selector: string): Element | null {
  return document.querySelector(`${selector}:not([hidden])`);
}

export function isHtmlFile(url: string): boolean {
  const path = new URL(url, location.origin).pathname;
  return path.endsWith('.html') || path.endsWith('.htm');
}

export function scrollOffset(): number {
  return 16;
}

export function formatRelative(ts: number): string {
  const diff = Date.now() / 1000 - ts;
  const p = (n: number, u: string) => `${n} ${u}${n === 1 ? '' : 's'} ago`;
  if (diff < 60) return 'just now';
  if (diff < 3600) return p(Math.floor(diff / 60), 'minute');
  if (diff < 86400) return p(Math.floor(diff / 3600), 'hour');
  if (diff < 7 * 86400) return p(Math.floor(diff / 86400), 'day');
  if (diff < 30 * 86400) return p(Math.floor(diff / (7 * 86400)), 'week');
  if (diff < 365 * 86400) return p(Math.floor(diff / (30 * 86400)), 'month');
  return p(Math.floor(diff / (365 * 86400)), 'year');
}

export function formatAbsolute(ts: number): string {
  return new Date(ts * 1000).toLocaleString('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
    timeZoneName: 'short',
  });
}

export function positionFloatingPanel(
  panel: HTMLElement,
  button: Element,
  fallbackWidth = 220,
): void {
  const rect = button.getBoundingClientRect();
  const width = panel.offsetWidth || fallbackWidth;
  const left = Math.max(
    16,
    Math.min(rect.right - width, window.innerWidth - width - 16),
  );
  panel.style.top = `${Math.round(rect.bottom + 8)}px`;
  panel.style.left = `${Math.round(left)}px`;
}

export function scrollToHash(hash: string): boolean {
  if (!hash || hash === '#') return false;
  const id = decodeURIComponent(hash.slice(1));
  const target = document.getElementById(id);
  if (!target) return false;
  const top =
    window.scrollY + target.getBoundingClientRect().top - scrollOffset();
  window.scrollTo({ top: Math.max(top, 0), behavior: 'auto' });
  return true;
}

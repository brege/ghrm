export function icon(name, cls = 'ghrm-file-icon') {
  return `<svg aria-hidden="true" height="16" width="16" class="${cls}"><use href="#ghrm-icon-${name}"></use></svg>`;
}

export function escapeHtml(value) {
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

export function visiblePane(selector) {
  return document.querySelector(`${selector}:not([hidden])`);
}

export function isHtmlFile(url) {
  const path = new URL(url, location.origin).pathname;
  return path.endsWith('.html') || path.endsWith('.htm');
}

export function scrollOffset() {
  return 16;
}

export function positionFloatingPanel(panel, button, fallbackWidth = 220) {
  const rect = button.getBoundingClientRect();
  const width = panel.offsetWidth || fallbackWidth;
  const left = Math.max(
    16,
    Math.min(rect.right - width, window.innerWidth - width - 16),
  );
  panel.style.top = `${Math.round(rect.bottom + 8)}px`;
  panel.style.left = `${Math.round(left)}px`;
}

export function scrollToHash(hash) {
  if (!hash || hash === '#') return false;
  const id = decodeURIComponent(hash.slice(1));
  const target = document.getElementById(id);
  if (!target) return false;
  const top =
    window.scrollY + target.getBoundingClientRect().top - scrollOffset();
  window.scrollTo({ top: Math.max(top, 0), behavior: 'auto' });
  return true;
}

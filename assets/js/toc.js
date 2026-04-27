import {
  positionFloatingPanel,
  scrollOffset,
  scrollToHash,
  visiblePane,
} from './dom.js';

function fileViewRoot() {
  return visiblePane('.ghrm-page-content [data-ghrm-preview-pane]');
}

function tocRoot() {
  const viewRoot = fileViewRoot();
  if (viewRoot) return viewRoot;
  if (document.querySelector('[data-ghrm-view-kind]')) return null;
  return (
    document.querySelector('article[data-explorer] .ghrm-readme-content') ||
    document.querySelector('article.markdown-body')
  );
}

function tocButton() {
  return document.querySelector('[data-ghrm-toc-btn]');
}

function syncTocButtons(show) {
  const btn = tocButton();
  for (const current of document.querySelectorAll('[data-ghrm-toc-btn]')) {
    current.hidden = current !== btn;
    current.disabled = current === btn ? !show : true;
  }
  return btn;
}

function headingText(heading) {
  const copy = heading.cloneNode(true);
  for (const anchor of copy.querySelectorAll('.ghrm-anchor')) {
    anchor.remove();
  }
  return copy.textContent.replace(/\s+/g, ' ').trim();
}

function currentHeadingId() {
  const root = tocRoot();
  if (!root) return '';
  const headings = [
    ...root.querySelectorAll('h1[id],h2[id],h3[id],h4[id],h5[id],h6[id]'),
  ];
  if (headings.length === 0) return '';

  const threshold = scrollOffset() + 12;
  let current = headings[0];
  for (const heading of headings) {
    if (window.scrollY + heading.getBoundingClientRect().top <= threshold) {
      current = heading;
    } else {
      break;
    }
  }
  return current.id;
}

function syncTocActive() {
  const panel = document.getElementById('ghrm-toc-panel');
  if (!panel) return;
  const activeId = currentHeadingId();
  for (const link of panel.querySelectorAll('a[href^="#"]')) {
    const href = decodeURIComponent(link.getAttribute('href') ?? '').slice(1);
    const active = href === activeId;
    link.classList.toggle('is-active', active);
    if (active) {
      link.setAttribute('aria-current', 'location');
    } else {
      link.removeAttribute('aria-current');
    }
  }
}

export function buildToc() {
  const panel = document.getElementById('ghrm-toc-panel');
  if (!panel) return;

  panel.hidden = true;
  panel.replaceChildren();

  const root = tocRoot();
  const headings = root
    ? [...root.querySelectorAll('h1[id],h2[id],h3[id],h4[id],h5[id],h6[id]')]
    : [];

  if (headings.length === 0) {
    syncTocButtons(false);
    return;
  }

  syncTocButtons(true);
  for (const heading of headings) {
    const text = headingText(heading);
    if (!text) continue;
    const link = document.createElement('a');
    link.className = `toc-h${heading.tagName[1]}`;
    link.href = `#${heading.id}`;
    link.textContent = text;
    panel.append(link);
  }
  syncTocActive();
}

function positionToc(panel, btn) {
  positionFloatingPanel(panel, btn, 248);
}

export function setupToc() {
  const panel = document.getElementById('ghrm-toc-panel');
  if (!panel) return;

  panel.addEventListener('click', (e) => {
    if (e.target.tagName === 'A') panel.hidden = true;
  });

  document.addEventListener('click', (e) => {
    const btn = e.target.closest('[data-ghrm-toc-btn]');
    if (btn) {
      if (btn.hidden || btn.disabled) return;
      buildToc();
      const nextHidden = !panel.hidden;
      panel.hidden = nextHidden;
      if (!nextHidden && panel.childElementCount > 0) {
        positionToc(panel, btn);
      }
      return;
    }
    if (!panel.contains(e.target)) {
      panel.hidden = true;
    }
  });

  window.addEventListener('resize', () => {
    if (panel.hidden) return;
    const btn = tocButton();
    if (btn) positionToc(panel, btn);
  });

  window.addEventListener('hashchange', () => {
    panel.hidden = true;
    scrollToHash(location.hash);
    syncTocActive();
  });

  window.addEventListener('scroll', syncTocActive, { passive: true });

  buildToc();
}

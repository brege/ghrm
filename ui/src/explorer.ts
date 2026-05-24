import {
  formatAbsolute,
  formatRelative,
  icon,
  isHtmlFile,
  qselAll,
  qselAllFrom,
  qselFrom,
} from './dom';

export function syncColumnControls(): void {
  const article = document.querySelector('article[data-explorer]');
  const controls = qselAll('[data-column-toggle].ghrm-view-option');
  const columns = new Set(
    controls
      .filter((control) => {
        return (
          control.dataset.columnToggle !== 'headers' &&
          control.classList.contains('is-active')
        );
      })
      .map((control) => control.dataset.columnToggle),
  );
  if (article) {
    const hasEdge = controls.some((control) => {
      return (
        control.dataset.columnToggle !== 'headers' &&
        control.dataset.columnEdge === '1' &&
        control.classList.contains('is-active')
      );
    });
    article.classList.toggle('ghrm-has-edge-meta', hasEdge);
    const cells = qselAllFrom(article, '[data-column-key]');
    for (const cell of cells) {
      cell.hidden = !columns.has(cell.dataset.columnKey);
    }
    const headers = qselFrom(article, '.ghrm-column-headers');
    const headerControl = controls.find((control) => {
      return control.dataset.columnToggle === 'headers';
    });
    if (headers) {
      headers.hidden = !headerControl?.classList.contains('is-active');
    }
  }
}

export function populateDates(): void {
  for (const el of qselAll('.ghrm-nav-meta-time[data-ts]')) {
    const ts = parseInt(el.dataset.ts || '', 10);
    if (!ts) continue;
    el.textContent = formatRelative(ts);
    el.title = formatAbsolute(ts);
  }
}

export function setupViewMenu(): void {
  syncColumnControls();
}

export function setupNavExternalLinks(): void {
  for (const row of document.querySelectorAll('.ghrm-nav-table tr')) {
    const nameLink = row.querySelector('.ghrm-nav-name a');
    const nameCell = nameLink?.closest('.ghrm-nav-name');
    if (!nameLink || !nameCell) continue;

    const href = nameLink.getAttribute('href');
    if (!href || !isHtmlFile(href)) continue;
    if (nameCell.querySelector('.ghrm-nav-external')) continue;

    const htmlHref = href.replace(/^\//, '/_ghrm/html/');
    const link = document.createElement('a');
    link.className = 'ghrm-nav-external';
    link.href = htmlHref;
    link.target = '_blank';
    link.rel = 'noopener noreferrer';
    link.dataset.ghrmNative = '1';
    link.setAttribute('aria-label', 'Open in browser');
    link.title = 'Open in browser';
    link.innerHTML = icon('external');
    nameLink.after(link);
  }
}

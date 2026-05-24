import {
  formatAbsolute,
  formatRelative,
  icon,
  isHtmlFile,
  positionFloatingPanel,
  qselAll,
  qselAllFrom,
  qselFrom,
} from './dom';
import type { GhrmArchiveProgress } from './islands/archive/progress';

interface ExplorerMenuConfig {
  name: string;
  toggleId: string;
  panelId: string;
}

interface ExplorerMenu extends ExplorerMenuConfig {
  toggle: HTMLElement;
  panel: HTMLElement;
}

let explorerMenusBound = false;

const EXPLORER_MENUS: ExplorerMenuConfig[] = [
  {
    name: 'filter',
    toggleId: 'ghrm-view-menu-toggle',
    panelId: 'ghrm-view-menu',
  },
  {
    name: 'sort',
    toggleId: 'ghrm-sort-menu-toggle',
    panelId: 'ghrm-sort-menu',
  },
  {
    name: 'archive',
    toggleId: 'ghrm-archive-menu-toggle',
    panelId: 'ghrm-archive-menu',
  },
  {
    name: 'column',
    toggleId: 'ghrm-column-menu-toggle',
    panelId: 'ghrm-column-menu',
  },
];

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

function closeExplorerMenus(): void {
  for (const { toggle, panel } of currentExplorerMenus()) {
    panel.hidden = true;
    toggle.setAttribute('aria-expanded', 'false');
  }
}

function currentExplorerMenus(): ExplorerMenu[] {
  return EXPLORER_MENUS.map((menu) => ({
    ...menu,
    toggle: document.getElementById(menu.toggleId),
    panel: document.getElementById(menu.panelId),
  })).filter((m): m is ExplorerMenu => m.toggle !== null && m.panel !== null);
}

function currentExplorerMenu(name: string): ExplorerMenu | null {
  return currentExplorerMenus().find((menu) => menu.name === name) || null;
}

function hasExplorerMenus(): boolean {
  return currentExplorerMenus().length === EXPLORER_MENUS.length;
}

function openExplorerMenu(name: string): void {
  const menu = currentExplorerMenu(name);
  if (!menu) return;
  closeExplorerMenus();
  menu.panel.hidden = false;
  menu.toggle.setAttribute('aria-expanded', 'true');
  positionFloatingPanel(menu.panel, menu.toggle);
}

export function setupViewMenu(): void {
  const filter = currentExplorerMenu('filter');
  const sort = currentExplorerMenu('sort');
  const archive = currentExplorerMenu('archive');
  const column = currentExplorerMenu('column');
  if (!filter || !sort || !archive || !column) return;

  syncColumnControls();
  closeExplorerMenus();

  for (const menu of currentExplorerMenus()) {
    menu.toggle.onclick = () => {
      if (menu.panel.hidden) {
        openExplorerMenu(menu.name);
      } else {
        closeExplorerMenus();
      }
    };
    for (const option of qselAllFrom(menu.panel, '.ghrm-view-option')) {
      option.onclick = (event) => {
        closeExplorerMenus();
        const archiveUrl = option.dataset.ghrmArchiveUrl;
        if (archiveUrl) {
          event.preventDefault();
          const progress = document.querySelector<GhrmArchiveProgress>(
            'ghrm-archive-progress',
          );
          progress?.startJob(archiveUrl);
        }
      };
    }
  }

  if (explorerMenusBound) {
    return;
  }
  explorerMenusBound = true;

  document.addEventListener('click', (e) => {
    const dirToggle = document.getElementById('ghrm-sort-dir-toggle');
    if (!hasExplorerMenus()) return;
    const target = e.target instanceof Node ? e.target : null;
    if (!target) return;
    const insideMenu = currentExplorerMenus().some(({ toggle, panel }) => {
      return toggle.contains(target) || panel.contains(target);
    });
    if (insideMenu || dirToggle?.contains(target)) return;
    closeExplorerMenus();
  });

  window.addEventListener('resize', () => {
    if (!hasExplorerMenus()) return;
    for (const { toggle, panel } of currentExplorerMenus()) {
      if (!panel.hidden) {
        positionFloatingPanel(panel, toggle);
      }
    }
  });

  document.addEventListener('keydown', (e) => {
    if (e.key !== 'Escape') return;
    if (!hasExplorerMenus()) return;
    const openMenu = currentExplorerMenus().find(({ panel }) => !panel.hidden);
    if (openMenu) {
      closeExplorerMenus();
      openMenu.toggle.focus();
    }
  });
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

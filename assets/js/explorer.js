import {
  formatAbsolute,
  formatRelative,
  icon,
  isHtmlFile,
  positionFloatingPanel,
} from './dom.js';

let explorerMenusBound = false;

const EXPLORER_MENUS = [
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
    name: 'column',
    toggleId: 'ghrm-column-menu-toggle',
    panelId: 'ghrm-column-menu',
  },
];

export function syncColumnControls() {
  const article = document.querySelector('article[data-explorer]');
  const controls = [
    ...document.querySelectorAll('[data-column-toggle].ghrm-view-option'),
  ];
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
    for (const cell of article.querySelectorAll('[data-column-key]')) {
      cell.hidden = !columns.has(cell.dataset.columnKey);
    }
    const headers = article.querySelector('.ghrm-column-headers');
    const headerControl = controls.find((control) => {
      return control.dataset.columnToggle === 'headers';
    });
    if (headers) {
      headers.hidden = !headerControl?.classList.contains('is-active');
    }
  }
}

export function populateDates() {
  for (const el of document.querySelectorAll('.ghrm-nav-meta-time[data-ts]')) {
    const ts = parseInt(el.dataset.ts, 10);
    if (!ts) continue;
    el.textContent = formatRelative(ts);
    el.title = formatAbsolute(ts);
  }
}

function closeExplorerMenus() {
  for (const { toggle, panel } of currentExplorerMenus()) {
    panel.hidden = true;
    toggle.setAttribute('aria-expanded', 'false');
  }
}

function currentExplorerMenus() {
  return EXPLORER_MENUS.map((menu) => ({
    ...menu,
    toggle: document.getElementById(menu.toggleId),
    panel: document.getElementById(menu.panelId),
  })).filter(({ toggle, panel }) => toggle && panel);
}

function currentExplorerMenu(name) {
  return currentExplorerMenus().find((menu) => menu.name === name) || null;
}

function hasExplorerMenus() {
  return currentExplorerMenus().length === EXPLORER_MENUS.length;
}

function openExplorerMenu(name) {
  const menu = currentExplorerMenu(name);
  if (!menu) return;
  closeExplorerMenus();
  menu.panel.hidden = false;
  menu.toggle.setAttribute('aria-expanded', 'true');
  positionFloatingPanel(menu.panel, menu.toggle);
}

export function setupViewMenu() {
  const filter = currentExplorerMenu('filter');
  const sort = currentExplorerMenu('sort');
  const column = currentExplorerMenu('column');
  if (!filter || !sort || !column) return;

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
    for (const option of menu.panel.querySelectorAll('.ghrm-view-option')) {
      option.onclick = () => {
        closeExplorerMenus();
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
    const insideMenu = currentExplorerMenus().some(({ toggle, panel }) => {
      return toggle.contains(e.target) || panel.contains(e.target);
    });
    if (insideMenu || dirToggle?.contains(e.target)) return;
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

export function setupNavExternalLinks() {
  for (const row of document.querySelectorAll('.ghrm-nav-table tr')) {
    const nameLink = row.querySelector('.ghrm-nav-name a');
    const nameCell = nameLink?.closest('.ghrm-nav-name');
    if (!nameLink || !nameCell) continue;

    const href = nameLink.getAttribute('href');
    if (!isHtmlFile(href)) continue;
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

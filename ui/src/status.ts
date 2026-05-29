import {
  formatAbsolute,
  positionFloatingPanel,
  qsel,
  qselAllFrom,
  qselFrom,
} from './dom';

let active = 0;
let connected = false;
let peekOpen = false;
let detailsOpen = false;
let aboutPanelMenuSetup = false;

const ABOUT_PANEL_PREF = 'ghrm-about-hidden-panels';
const ABOUT_PANELS = new Set([
  'scope',
  'directory',
  'filters',
  'paths',
  'network',
]);

function populateAboutTitles(root: Document | Element = document): void {
  for (const el of qselAllFrom(root, '[data-ghrm-title-ts]')) {
    const raw = el.dataset.ghrmTitleTs;
    if (!raw) continue;
    const ts = parseInt(raw, 10);
    if (!ts) continue;
    el.title = formatAbsolute(ts);
  }
}

function validAboutPanel(value: string | undefined): string | null {
  if (value && ABOUT_PANELS.has(value)) {
    return value;
  }
  return null;
}

function readHiddenAboutPanels(): Set<string> {
  const raw = localStorage.getItem(ABOUT_PANEL_PREF);
  if (!raw) {
    return new Set();
  }
  try {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) {
      return new Set();
    }
    return new Set(
      parsed.filter(
        (value): value is string =>
          typeof value === 'string' && ABOUT_PANELS.has(value),
      ),
    );
  } catch {
    return new Set();
  }
}

function writeHiddenAboutPanels(hidden: Set<string>): void {
  localStorage.setItem(ABOUT_PANEL_PREF, JSON.stringify([...hidden].sort()));
}

function aboutPanelToggle(): HTMLElement | null {
  return qsel('#ghrm-about-panel-menu-toggle');
}

function aboutPanelMenu(): HTMLElement | null {
  return qsel('#ghrm-about-panel-menu');
}

function closeAboutPanelMenu(focus = false): void {
  const toggle = aboutPanelToggle();
  const menu = aboutPanelMenu();
  if (!toggle || !menu) return;
  menu.hidden = true;
  toggle.setAttribute('aria-expanded', 'false');
  if (focus) {
    toggle.focus();
  }
}

function openAboutPanelMenu(): void {
  const toggle = aboutPanelToggle();
  const menu = aboutPanelMenu();
  if (!toggle || !menu) return;
  menu.hidden = false;
  toggle.setAttribute('aria-expanded', 'true');
  positionFloatingPanel(menu, toggle, 210);
}

function syncDetailGrids(root: Document | Element = document): void {
  for (const grid of qselAllFrom(root, '.ghrm-detail-grid')) {
    const panels = [...grid.children].filter(
      (child): child is HTMLElement => child instanceof HTMLElement,
    );
    grid.hidden = panels.length > 0 && panels.every((panel) => panel.hidden);
  }
}

export function applyAboutPanelPrefs(
  root: Document | Element = document,
): void {
  const hidden = readHiddenAboutPanels();
  for (const panel of qselAllFrom(root, '[data-ghrm-about-panel]')) {
    const name = validAboutPanel(panel.dataset.ghrmAboutPanel);
    if (name) {
      panel.hidden = hidden.has(name);
    }
  }
  for (const option of qselAllFrom(root, '[data-ghrm-about-panel-option]')) {
    const name = validAboutPanel(option.dataset.ghrmAboutPanelOption);
    const panel = name
      ? document.querySelector(`[data-ghrm-about-panel="${name}"]`)
      : null;
    if (!name || !panel) {
      option.hidden = true;
      continue;
    }
    const shown = !hidden.has(name);
    option.hidden = false;
    option.classList.toggle('is-active', shown);
    option.setAttribute('aria-checked', shown ? 'true' : 'false');
  }
  syncDetailGrids(root);
}

export function toggleAboutPanel(name: string): void {
  const panel = validAboutPanel(name);
  if (!panel) return;
  const hidden = readHiddenAboutPanels();
  if (hidden.has(panel)) {
    hidden.delete(panel);
  } else {
    hidden.add(panel);
  }
  writeHiddenAboutPanels(hidden);
  applyAboutPanelPrefs();
}

export function setupAboutPanelMenu(): void {
  applyAboutPanelPrefs();
  if (aboutPanelMenuSetup) return;
  aboutPanelMenuSetup = true;

  document.addEventListener('click', (event) => {
    const target = event.target instanceof Element ? event.target : null;
    if (!target) return;

    const option = target.closest('[data-ghrm-about-panel-option]');
    if (option instanceof HTMLElement) {
      event.preventDefault();
      const name = validAboutPanel(option.dataset.ghrmAboutPanelOption);
      if (name) {
        toggleAboutPanel(name);
      }
      return;
    }

    const toggle = target.closest('#ghrm-about-panel-menu-toggle');
    if (toggle) {
      event.preventDefault();
      const menu = aboutPanelMenu();
      if (menu?.hidden) {
        openAboutPanelMenu();
      } else {
        closeAboutPanelMenu();
      }
      return;
    }

    const menu = aboutPanelMenu();
    if (!menu || menu.hidden) return;
    if (target.closest('#ghrm-about-panel-menu')) return;
    closeAboutPanelMenu();
  });

  document.addEventListener('keydown', (event) => {
    if (event.key !== 'Escape') return;
    const menu = aboutPanelMenu();
    if (!menu || menu.hidden) return;
    closeAboutPanelMenu(true);
  });

  window.addEventListener('resize', () => {
    const menu = aboutPanelMenu();
    const toggle = aboutPanelToggle();
    if (!menu || !toggle || menu.hidden) return;
    positionFloatingPanel(menu, toggle, 210);
  });
}

function holdPeekHeight(peek: HTMLElement, path: string): number {
  if (!peekOpen || peek.hidden) {
    return 0;
  }
  const height = Math.ceil(peek.getBoundingClientRect().height);
  if (height <= 0) {
    return 0;
  }
  peek.style.minHeight = `${height}px`;
  peek.dataset.heightHold = path;
  return height;
}

function releasePeekHeight(peek: HTMLElement, path: string): void {
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      if (peek.dataset.heightHold !== path) {
        return;
      }
      delete peek.dataset.heightHold;
      peek.style.minHeight = '';
    });
  });
}

async function loadAboutPeek(): Promise<void> {
  const peek = qsel('#ghrm-about-peek');
  const path = window.location.pathname || '/';
  const statsPath = `${path}${window.location.search || ''}`;
  if (
    !peek ||
    (peek.dataset.statsLoaded === 'true' &&
      peek.dataset.statsPath === statsPath) ||
    peek.dataset.statsLoading === statsPath
  ) {
    return;
  }

  peek.dataset.statsLoading = statsPath;
  const heldHeight = holdPeekHeight(peek, statsPath);
  const params = new URLSearchParams(window.location.search);
  params.set('path', path);
  beginActivity();
  try {
    const response = await fetch(`/_ghrm/about?${params.toString()}`, {
      headers: { Accept: 'text/html' },
    });
    if (!response.ok) {
      return;
    }
    const template = document.createElement('template');
    template.innerHTML = (await response.text()).trim();
    const nextEl = template.content.firstElementChild;
    if (!(nextEl instanceof HTMLElement) || nextEl.id !== 'ghrm-about-peek') {
      return;
    }
    const next = nextEl;
    if (
      `${window.location.pathname || '/'}${window.location.search || ''}` !==
      statsPath
    ) {
      return;
    }
    next.hidden = !peekOpen;
    next.dataset.statsPath = statsPath;
    if (heldHeight > 0) {
      next.style.minHeight = `${heldHeight}px`;
      next.dataset.heightHold = statsPath;
    }
    populateAboutTitles(next);
    peek.replaceWith(next);
  } finally {
    const current = qsel('#ghrm-about-peek');
    if (current?.dataset.statsLoading === statsPath) {
      delete current.dataset.statsLoading;
    }
    if (current && heldHeight > 0) {
      releasePeekHeight(current, statsPath);
    }
    endActivity();
    sync();
  }
}

function sync(): void {
  const source = document.getElementById('ghrm-source-slot');
  const button = source?.querySelector('.ghrm-source-badge');
  const peek = qsel('#ghrm-about-peek');
  const detailsButton = peek
    ? qselFrom(peek, '.ghrm-about-stamp-button')
    : null;

  if (source) {
    source.classList.toggle('is-active', active > 0);
    source.classList.toggle('is-muted', !connected && active === 0);
  }
  if (button) {
    button.setAttribute('aria-expanded', peekOpen ? 'true' : 'false');
  }
  if (peek) {
    peek.hidden = !peekOpen;
    peek.classList.toggle('is-details-open', detailsOpen);
    applyAboutPanelPrefs(peek);
    if (!peekOpen) {
      closeAboutPanelMenu();
    }
  }
  if (detailsButton) {
    const detailsLabel = detailsOpen
      ? 'Hide runtime details'
      : 'Show runtime details';
    detailsButton.setAttribute('aria-expanded', detailsOpen ? 'true' : 'false');
    detailsButton.setAttribute('aria-label', detailsLabel);
    detailsButton.title = detailsLabel;
  }
  document.body?.classList.toggle('ghrm-about-open', peekOpen);
}

export function beginActivity(): void {
  active += 1;
  sync();
}

export function endActivity(): void {
  active = Math.max(0, active - 1);
  sync();
}

export function setConnected(value: boolean): void {
  connected = value;
  sync();
}

export function syncServerStatus(): void {
  if (peekOpen) {
    void loadAboutPeek();
  }
  sync();
}

export function setupStatusPeek(): void {
  setupAboutPanelMenu();
  document.addEventListener('click', (event) => {
    const target = event.target instanceof Element ? event.target : null;
    if (!target) return;
    if (target.closest('.ghrm-about-stamp-button')) {
      event.preventDefault();
      detailsOpen = !detailsOpen;
      sync();
      return;
    }

    if (!target.closest('.ghrm-source-badge')) return;
    event.preventDefault();
    peekOpen = !peekOpen;
    if (peekOpen) {
      void loadAboutPeek();
    }
    sync();
  });
  populateAboutTitles();
  sync();
}

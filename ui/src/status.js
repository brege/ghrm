import { formatAbsolute, qsel, qselAllFrom, qselFrom } from './dom';

let active = 0;
let connected = false;
let peekOpen = false;
let detailsOpen = false;

/**
 * @param {Document | Element} root
 */
function populateAboutTitles(root = document) {
  for (const el of qselAllFrom(root, '[data-ghrm-title-ts]')) {
    const ts = parseInt(el.dataset.ghrmTitleTs, 10);
    if (!ts) continue;
    el.title = formatAbsolute(ts);
  }
}

/**
 * @param {HTMLElement} peek
 * @param {string} path
 */
function holdPeekHeight(peek, path) {
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

/**
 * @param {HTMLElement} peek
 * @param {string} path
 */
function releasePeekHeight(peek, path) {
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

async function loadAboutPeek() {
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

function sync() {
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

export function beginActivity() {
  active += 1;
  sync();
}

export function endActivity() {
  active = Math.max(0, active - 1);
  sync();
}

export function setConnected(value) {
  connected = value;
  sync();
}

export function syncServerStatus() {
  if (peekOpen) {
    void loadAboutPeek();
  }
  sync();
}

export function setupStatusPeek() {
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

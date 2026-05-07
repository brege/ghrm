import { formatAbsolute } from './dom.js';

let active = 0;
let connected = false;
let peekOpen = false;
let detailsOpen = false;

function populateAboutTitles(root = document) {
  for (const el of root.querySelectorAll('[data-ghrm-title-ts]')) {
    const ts = parseInt(el.dataset.ghrmTitleTs, 10);
    if (!ts) continue;
    el.title = formatAbsolute(ts);
  }
}

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
  const peek = document.getElementById('ghrm-about-peek');
  const path = window.location.pathname || '/';
  if (
    !peek ||
    (peek.dataset.statsLoaded === 'true' && peek.dataset.statsPath === path) ||
    peek.dataset.statsLoading === path
  ) {
    return;
  }

  peek.dataset.statsLoading = path;
  const heldHeight = holdPeekHeight(peek, path);
  beginActivity();
  try {
    const response = await fetch(
      `/_ghrm/about?path=${encodeURIComponent(path)}`,
      {
        headers: { Accept: 'text/html' },
      },
    );
    if (!response.ok) {
      return;
    }
    const template = document.createElement('template');
    template.innerHTML = (await response.text()).trim();
    const next = template.content.firstElementChild;
    if (next?.id !== 'ghrm-about-peek') {
      return;
    }
    if ((window.location.pathname || '/') !== path) {
      return;
    }
    next.hidden = !peekOpen;
    next.dataset.statsPath = path;
    if (heldHeight > 0) {
      next.style.minHeight = `${heldHeight}px`;
      next.dataset.heightHold = path;
    }
    populateAboutTitles(next);
    peek.replaceWith(next);
  } finally {
    const current = document.getElementById('ghrm-about-peek');
    if (current?.dataset.statsLoading === path) {
      delete current.dataset.statsLoading;
    }
    if (current && heldHeight > 0) {
      releasePeekHeight(current, path);
    }
    endActivity();
    sync();
  }
}

function sync() {
  const source = document.getElementById('ghrm-source-slot');
  const button = source?.querySelector('.ghrm-source-badge');
  const peek = document.getElementById('ghrm-about-peek');
  const detailsButton = peek?.querySelector('.ghrm-about-stamp-button');

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
    detailsButton.setAttribute('aria-expanded', detailsOpen ? 'true' : 'false');
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
    if (event.target.closest('.ghrm-about-stamp-button')) {
      event.preventDefault();
      detailsOpen = !detailsOpen;
      sync();
      return;
    }

    if (!event.target.closest('.ghrm-source-badge')) return;
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

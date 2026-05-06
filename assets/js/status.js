let active = 0;
let connected = false;
let peekOpen = false;
let detailsOpen = false;

async function loadAboutPeek() {
  const peek = document.getElementById('ghrm-about-peek');
  if (
    !peek ||
    peek.dataset.statsLoaded === 'true' ||
    peek.dataset.statsLoading === 'true'
  ) {
    return;
  }

  peek.dataset.statsLoading = 'true';
  beginActivity();
  try {
    const path = window.location.pathname || '/';
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
    next.hidden = !peekOpen;
    peek.replaceWith(next);
  } finally {
    const current = document.getElementById('ghrm-about-peek');
    if (current?.dataset.statsLoaded !== 'true') {
      delete current?.dataset.statsLoading;
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
  sync();
}

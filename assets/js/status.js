let active = 0;
let connected = false;
let peekOpen = false;

function sync() {
  const source = document.getElementById('ghrm-source-slot');
  const button = source?.querySelector('.ghrm-source-badge');
  const peek = document.getElementById('ghrm-about-peek');

  if (source) {
    source.classList.toggle('is-active', active > 0);
    source.classList.toggle('is-muted', !connected && active === 0);
  }
  if (button) {
    button.setAttribute('aria-expanded', peekOpen ? 'true' : 'false');
  }
  if (peek) {
    peek.hidden = !peekOpen;
  }
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
  sync();
}

export function setupStatusPeek() {
  document.addEventListener('click', (event) => {
    if (!event.target.closest('.ghrm-source-badge')) return;
    event.preventDefault();
    peekOpen = !peekOpen;
    sync();
  });
  sync();
}

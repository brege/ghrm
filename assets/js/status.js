let active = 0;
let connected = false;

function slot() {
  return document.getElementById('ghrm-source-slot');
}

function syncStatus() {
  const el = slot();
  if (!el) return;
  el.classList.toggle('is-active', active > 0);
  el.classList.toggle('is-muted', !connected && active === 0);
}

export function beginActivity() {
  active += 1;
  syncStatus();
}

export function endActivity() {
  active = Math.max(0, active - 1);
  syncStatus();
}

export function setConnected(value) {
  connected = value;
  syncStatus();
}

export function syncServerStatus() {
  syncStatus();
}

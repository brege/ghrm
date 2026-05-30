export function setupThemeToggle(): void {
  const btn = document.getElementById('theme-toggle');
  if (!btn) return;
  btn.addEventListener('click', () => {
    const current = document.documentElement.getAttribute('data-theme');
    const next = current === 'dark' ? 'light' : 'dark';
    document.documentElement.setAttribute('data-theme', next);
    localStorage.setItem('ghrm-theme', next);
    document.dispatchEvent(
      new CustomEvent('ghrm:themechange', { detail: { theme: next } }),
    );
  });
}

function hasDocChrome(): boolean {
  return !!document.querySelector('.ghrm-page-shell[data-ghrm-view-kind]');
}

function printHref(next: boolean): string {
  const url = new URL(window.location.href);
  if (next) {
    url.searchParams.set('print', '1');
  } else {
    url.searchParams.delete('print');
  }
  return `${url.pathname}${url.search}${url.hash}`;
}

export function isPrintMode(): boolean {
  const value = new URLSearchParams(window.location.search).get('print');
  return hasDocChrome() && (value === '1' || value === 'true');
}

export function syncPrintMode(): void {
  const print = isPrintMode();
  document.body.classList.toggle('ghrm-print', print);
  applyWrapState(print || getWrapPref());
}

function syncDocChromeToggle(): void {
  const btn = document.getElementById('doc-chrome-toggle');
  if (!btn) return;
  const show = hasDocChrome();
  btn.hidden = !show;
  if (!show) {
    btn.removeAttribute('title');
    btn.removeAttribute('aria-label');
    return;
  }
  const print = isPrintMode();
  const label = print ? 'Show document wrapper' : 'Hide document wrapper';
  btn.title = label;
  btn.setAttribute('aria-label', label);
}

export function applyDocChromePref(): void {
  document.body.classList.remove('ghrm-doc-flat');
  syncDocChromeToggle();
}

export function setupDocChromeToggle(): void {
  const btn = document.getElementById('doc-chrome-toggle');
  if (!btn) return;
  btn.addEventListener('click', () => {
    window.location.assign(printHref(!isPrintMode()));
  });
  applyDocChromePref();
}

export function getWrapPref(): boolean {
  return localStorage.getItem('ghrm-wrap') === '1';
}

export function setWrapPref(wrap: boolean): void {
  localStorage.setItem('ghrm-wrap', wrap ? '1' : '0');
}

export function applyWrapState(wrap: boolean): void {
  document.body.classList.toggle('ghrm-wrap', wrap);
}

export function setupThemeToggle() {
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

function hasDocChrome() {
  return !!document.querySelector('.ghrm-page-shell[data-ghrm-view-kind]');
}

function syncDocChromeToggle() {
  const btn = document.getElementById('doc-chrome-toggle');
  if (!btn) return;
  const show = hasDocChrome();
  btn.hidden = !show;
  if (!show) {
    btn.removeAttribute('title');
    btn.removeAttribute('aria-label');
    return;
  }
  const flat = document.body.classList.contains('ghrm-doc-flat');
  const label = flat ? 'Show document wrapper' : 'Hide document wrapper';
  btn.title = label;
  btn.setAttribute('aria-label', label);
}

export function applyDocChromePref() {
  const flat = localStorage.getItem('ghrm-doc-flat') === '1';
  document.body.classList.toggle('ghrm-doc-flat', flat && hasDocChrome());
  syncDocChromeToggle();
}

export function setupDocChromeToggle() {
  const btn = document.getElementById('doc-chrome-toggle');
  if (!btn) return;
  btn.addEventListener('click', () => {
    const next = !document.body.classList.contains('ghrm-doc-flat');
    document.body.classList.toggle('ghrm-doc-flat', next && hasDocChrome());
    localStorage.setItem('ghrm-doc-flat', next ? '1' : '0');
    syncDocChromeToggle();
  });
  applyDocChromePref();
}

export function getWrapPref() {
  return localStorage.getItem('ghrm-wrap') === '1';
}

export function setWrapPref(wrap) {
  localStorage.setItem('ghrm-wrap', wrap ? '1' : '0');
}

export function applyWrapState(wrap) {
  document.body.classList.toggle('ghrm-wrap', wrap);
}

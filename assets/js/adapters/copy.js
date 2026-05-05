import { icon } from '../dom.js';

// ghrm is often opened over plain HTTP from another LAN device. Browsers treat
// localhost as a secure context, but not http://192.168.x.x, so the Clipboard
// API may fail on Android or a second machine even when it works on the host.
// Use the platform API first, then fall back to user-initiated textarea copy.
const copyResetDelay = 1000;

export function copyIcon() {
  return icon('copy', 'ghrm-copy-icon ghrm-copy-icon-copy');
}

export function checkIcon() {
  return icon('check', 'ghrm-copy-icon ghrm-copy-icon-check');
}

function getCopyHost(pre) {
  const wrapper = pre.parentElement;
  if (wrapper?.classList.contains('highlight')) {
    return wrapper;
  }

  return pre;
}

function getCopyText(pre) {
  return pre.querySelector('code')?.textContent || pre.textContent || '';
}

export async function writeClipboard(text) {
  if (navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text);
      return;
    } catch {
      fallbackCopy(text);
      return;
    }
  }

  fallbackCopy(text);
}

function fallbackCopy(text) {
  const area = document.createElement('textarea');
  area.value = text;
  area.setAttribute('readonly', '');
  area.style.position = 'fixed';
  area.style.inset = '0 auto auto 0';
  area.style.width = '1px';
  area.style.height = '1px';
  area.style.opacity = '0';
  document.body.appendChild(area);
  area.select();
  area.setSelectionRange(0, area.value.length);
  const ok = document.execCommand('copy');
  area.remove();
  if (!ok) {
    throw new Error('Clipboard copy failed');
  }
}

export function showCopied(button) {
  if (button._ghrmCopyReset) {
    window.clearTimeout(button._ghrmCopyReset);
  }

  button.classList.add('is-copied');
  const feedback = button.dataset.copyFeedback || 'Copied!';
  button.setAttribute('aria-label', feedback);
  button.title = feedback;

  button._ghrmCopyReset = window.setTimeout(() => {
    button.classList.remove('is-copied');
    const label = button.dataset.copyLabel || 'Copy';
    button.setAttribute('aria-label', label);
    button.title = label;
    button._ghrmCopyReset = null;
  }, copyResetDelay);
}

export function addCopyButtons() {
  for (const pre of document.querySelectorAll('.markdown-body pre')) {
    if (pre.closest('[data-ghrm-raw-pane]')) {
      continue;
    }

    const host = getCopyHost(pre);
    if (!host || host.querySelector(':scope > .ghrm-copy-button')) {
      continue;
    }

    host.classList.add('ghrm-copy-host');
    pre.classList.add('ghrm-copy-target');

    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'ghrm-copy-button';
    button.setAttribute('aria-label', 'Copy');
    button.dataset.copyLabel = 'Copy';
    button.dataset.copyFeedback = 'Copied!';
    button.title = 'Copy';
    button.innerHTML = `${copyIcon()}${checkIcon()}`;
    button.addEventListener('click', async () => {
      await writeClipboard(getCopyText(pre));
      showCopied(button);
    });

    host.appendChild(button);
  }
}

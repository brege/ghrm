import { icon } from '../dom.js';

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
  if (!navigator.clipboard?.writeText) {
    throw new Error('Clipboard API unavailable');
  }
  await navigator.clipboard.writeText(text);
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

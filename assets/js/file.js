import {
  checkIcon,
  copyIcon,
  showCopied,
  writeClipboard,
} from './adapters/copy.js';
import { icon, isHtmlFile } from './dom.js';
import { applyWrapState, getWrapPref, setWrapPref } from './prefs.js';
import { buildToc } from './toc.js';

function rawText(container) {
  return (
    container.querySelector('[data-ghrm-raw-pane] .ghrm-data')?.content
      ?.textContent ||
    container.querySelector('[data-ghrm-raw-pane] code')?.textContent ||
    ''
  );
}

function syncFileView(container, raw) {
  const preview = container.querySelector('[data-ghrm-preview-pane]');
  const rawPane = container.querySelector('[data-ghrm-raw-pane]');
  const toggle = container.querySelector('[data-ghrm-raw-toggle]');
  if (!preview || !rawPane || !toggle) return;

  preview.hidden = raw;
  rawPane.hidden = !raw;
  toggle.classList.toggle('is-active', raw);
  toggle.setAttribute('aria-pressed', raw ? 'true' : 'false');

  const label = raw ? 'Show preview' : 'Show raw';
  toggle.setAttribute('aria-label', label);
  toggle.title = label;

  syncWrapToggle(container, raw);
}

function syncWrapToggle(container, isRaw) {
  const wrapToggle = container.querySelector('[data-ghrm-wrap-toggle]');
  if (!wrapToggle) return;

  const disabled = !isRaw;
  wrapToggle.disabled = disabled;

  if (disabled) {
    wrapToggle.classList.remove('is-active');
    wrapToggle.setAttribute('aria-pressed', 'false');
    wrapToggle.setAttribute('aria-label', 'Wrap lines (code view only)');
    wrapToggle.title = 'Wrap lines (code view only)';
    applyWrapState(false);
  } else {
    const wrap = getWrapPref();
    wrapToggle.classList.toggle('is-active', wrap);
    wrapToggle.setAttribute('aria-pressed', wrap ? 'true' : 'false');
    const label = wrap ? 'Disable line wrap' : 'Wrap lines';
    wrapToggle.setAttribute('aria-label', label);
    wrapToggle.title = label;
    applyWrapState(wrap);
  }
}

function fileActionsHost(container) {
  return container.querySelector('.ghrm-explorer-header .ghrm-header-actions');
}

function setupFileView(container) {
  const kind = container.dataset.ghrmViewKind;
  const rawUrl = container.dataset.ghrmRawUrl;
  const downloadUrl = container.dataset.ghrmDownloadUrl;
  const host = fileActionsHost(container);
  if (!kind || !rawUrl || !downloadUrl || !host) return;
  if (host.querySelector('.ghrm-file-tools')) return;

  const tools = document.createElement('div');
  tools.className = 'ghrm-file-tools';

  const toggles = document.createElement('div');
  toggles.className = 'ghrm-file-toggles';

  const toggle = document.createElement('button');
  toggle.type = 'button';
  toggle.className = 'ghrm-file-toggle';
  toggle.dataset.ghrmRawToggle = '1';
  toggle.innerHTML = icon('code');

  if (kind === 'markdown') {
    toggle.addEventListener('click', () => {
      const raw = toggle.getAttribute('aria-pressed') !== 'true';
      syncFileView(container, raw);
      buildToc();
      const panel = document.getElementById('ghrm-toc-panel');
      if (panel) {
        panel.hidden = true;
      }
    });
  } else {
    toggle.disabled = true;
  }

  const wrapToggle = document.createElement('button');
  wrapToggle.type = 'button';
  wrapToggle.className = 'ghrm-file-toggle';
  wrapToggle.dataset.ghrmWrapToggle = '1';
  wrapToggle.innerHTML = icon('wrap');

  wrapToggle.addEventListener('click', () => {
    setWrapPref(!getWrapPref());
    syncWrapToggle(container, true);
  });

  toggles.append(toggle, wrapToggle);

  if (isHtmlFile(rawUrl)) {
    const htmlUrl = rawUrl.replace('/_ghrm/raw/', '/_ghrm/html/');
    const external = document.createElement('a');
    external.className = 'ghrm-file-toggle';
    external.href = htmlUrl;
    external.target = '_blank';
    external.rel = 'noopener noreferrer';
    external.dataset.ghrmNative = '1';
    external.setAttribute('aria-label', 'Open in browser');
    external.title = 'Open in browser';
    external.innerHTML = icon('external');
    toggles.append(external);
  }

  const actions = document.createElement('div');
  actions.className = 'ghrm-file-actions';

  const rawLink = document.createElement('a');
  rawLink.className = 'ghrm-file-link';
  rawLink.href = rawUrl;
  rawLink.textContent = 'Raw';
  rawLink.target = '_blank';
  rawLink.rel = 'noopener noreferrer';
  rawLink.dataset.ghrmNative = '1';
  rawLink.setAttribute('aria-label', 'Open raw file');
  rawLink.title = 'Open raw file';

  const copy = document.createElement('button');
  copy.type = 'button';
  copy.className = 'ghrm-file-action';
  copy.innerHTML = `${copyIcon()}${checkIcon()}`;
  copy.dataset.copyLabel = 'Copy raw file';
  copy.dataset.copyFeedback = 'Copied!';
  copy.setAttribute('aria-label', 'Copy raw file');
  copy.title = 'Copy raw file';
  copy.addEventListener('click', async () => {
    await writeClipboard(rawText(container));
    showCopied(copy);
  });

  const download = document.createElement('a');
  download.className = 'ghrm-file-action';
  download.href = downloadUrl;
  download.dataset.ghrmNative = '1';
  download.setAttribute('download', '');
  download.setAttribute('aria-label', 'Download raw file');
  download.title = 'Download raw file';
  download.innerHTML = icon('download');

  actions.append(rawLink, copy, download);
  tools.append(toggles, actions);
  host.prepend(tools);
  syncFileView(container, kind === 'raw');
}

export function setupFileViews() {
  for (const container of document.querySelectorAll(
    '.ghrm-page-shell[data-ghrm-view-kind]',
  )) {
    setupFileView(container);
  }
}

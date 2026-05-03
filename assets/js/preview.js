import { escapeHtml } from './dom.js';

let mermaidId = 0;
let mermaidVersionPromise;
const copyResetDelay = 1000;
const vendorLoading = new Map();
let assetConfig;

function loadScript(src) {
  if (vendorLoading.has(src)) return vendorLoading.get(src);
  if (document.querySelector(`script[src="${src}"]`)) return Promise.resolve();
  const promise = new Promise((resolve, reject) => {
    const script = document.createElement('script');
    script.src = src;
    script.onload = resolve;
    script.onerror = reject;
    document.head.appendChild(script);
  });
  vendorLoading.set(src, promise);
  return promise;
}

function loadStylesheet(href) {
  if (document.querySelector(`link[href="${href}"]`)) return;
  const link = document.createElement('link');
  link.rel = 'stylesheet';
  link.href = href;
  document.head.appendChild(link);
}

function currentArticle() {
  return document.querySelector('article.markdown-body');
}

function assetPlan() {
  if (!assetConfig) {
    assetConfig = JSON.parse(
      document.getElementById('ghrm-assets')?.textContent || '{"features":{}}',
    );
  }
  return assetConfig;
}

function currentFeatures() {
  return (currentArticle()?.dataset.ghrmFeatures || '')
    .split(/\s+/)
    .filter((value) => value);
}

function hasFeature(name) {
  return currentFeatures().includes(name);
}

async function loadAssets() {
  const config = assetPlan();
  for (const name of currentFeatures()) {
    const feature = config.features?.[name];
    if (!feature) continue;
    for (const href of feature.styles || []) loadStylesheet(href);
    for (const src of feature.scripts || []) await loadScript(src);
  }
}

const SHELL_BUILTINS = new Set([
  '.',
  ':',
  'alias',
  'bg',
  'bind',
  'break',
  'builtin',
  'caller',
  'cd',
  'command',
  'compgen',
  'complete',
  'compopt',
  'continue',
  'declare',
  'dirs',
  'disown',
  'echo',
  'enable',
  'eval',
  'exec',
  'exit',
  'export',
  'false',
  'fc',
  'fg',
  'getopts',
  'hash',
  'help',
  'history',
  'jobs',
  'kill',
  'let',
  'local',
  'logout',
  'mapfile',
  'popd',
  'printf',
  'pushd',
  'pwd',
  'read',
  'readarray',
  'readonly',
  'return',
  'set',
  'shift',
  'shopt',
  'source',
  'suspend',
  'test',
  'times',
  'trap',
  'true',
  'type',
  'typeset',
  'ulimit',
  'umask',
  'unalias',
  'unset',
  'wait',
]);

function icon(name, cls = '') {
  const classes = cls ? `${cls}` : 'ghrm-action-icon';
  return `<svg aria-hidden="true" height="16" width="16" class="${classes}"><use href="#ghrm-icon-${name}"></use></svg>`;
}

export function copyIcon() {
  return icon('copy', 'ghrm-copy-icon ghrm-copy-icon-copy');
}

export function checkIcon() {
  return icon('check', 'ghrm-copy-icon ghrm-copy-icon-check');
}

function fullscreenIcon() {
  return icon('fullscreen');
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

function addCopyButtons() {
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

function getSource(block) {
  return block.querySelector('.ghrm-data')?.content?.textContent?.trim() || '';
}

function isDarkTheme() {
  return document.documentElement.getAttribute('data-theme') === 'dark';
}

function setError(block, message) {
  let node = block.querySelector('.ghrm-error');
  if (!node) {
    node = document.createElement('p');
    node.className = 'ghrm-error';
    block.appendChild(node);
  }
  node.hidden = false;
  node.textContent = message;
}

function clearError(block) {
  const node = block.querySelector('.ghrm-error');
  if (node) {
    node.hidden = true;
    node.textContent = '';
  }
}

function themeColors() {
  return {
    polygon: '#6f42c1',
    polygonFill: '#6f42c1',
    line: '#0969da',
    point: '#0969da',
  };
}

function mermaidTheme() {
  if (isDarkTheme()) {
    return {
      theme: 'base',
      themeVariables: {
        primaryColor: '#1f2020',
        primaryBorderColor: '#ccc',
        primaryTextColor: '#e6edf3',
        lineColor: '#ccc',
        secondaryColor: '#1f2020',
        tertiaryColor: '#1f2020',
        mainBkg: '#1f2020',
        nodeBkg: '#1f2020',
        nodeBorder: '#ccc',
        clusterBkg: '#161b22',
        clusterBorder: '#ccc',
        edgeLabelBackground: '#0d1117',
      },
    };
  }

  return {
    theme: 'neutral',
    themeVariables: {
      primaryColor: '#eae4f5',
      primaryBorderColor: '#998eb5',
      primaryTextColor: '#1f2328',
      lineColor: '#666',
      secondaryColor: '#eae4f5',
      tertiaryColor: '#eae4f5',
    },
  };
}

function restoreGitHubInlineMath(container) {
  for (const code of container.querySelectorAll('code')) {
    if (code.closest('pre')) {
      continue;
    }

    const previous = code.previousSibling;
    const next = code.nextSibling;
    if (
      previous?.nodeType !== Node.TEXT_NODE ||
      next?.nodeType !== Node.TEXT_NODE
    ) {
      continue;
    }

    const before = previous.textContent || '';
    const after = next.textContent || '';
    if (!before.endsWith('$') || !after.startsWith('$')) {
      continue;
    }

    const math = document.createTextNode(
      `${before.slice(0, -1)}$${code.textContent || ''}$${after.slice(1)}`,
    );
    code.replaceWith(math);
    previous.textContent = before.slice(0, -1);
    next.textContent = after.slice(1);
    previous.remove();
    next.remove();
  }
}

async function renderMath() {
  if (!hasFeature('math')) return;

  const containers = document.querySelectorAll('.markdown-body');
  if (containers.length === 0) return;

  if (typeof window.renderMathInElement !== 'function') return;

  for (const container of containers) {
    // GitHub's $`...`$ form becomes $<code>...</code>$ after Markdown parsing.
    restoreGitHubInlineMath(container);
    window.renderMathInElement(container, {
      delimiters: [
        { left: '$$', right: '$$', display: true },
        { left: '$`', right: '`$', display: false },
        { left: '$', right: '$', display: false },
        { left: '\\(', right: '\\)', display: false },
        { left: '\\[', right: '\\]', display: true },
      ],
      throwOnError: false,
      ignoredTags: ['script', 'noscript', 'style', 'textarea', 'pre', 'code'],
    });
  }
}

function renderCode() {
  if (typeof window.hljs?.highlightElement !== 'function') {
    return;
  }

  for (const code of document.querySelectorAll('.markdown-body pre code')) {
    const hasLanguage = [...code.classList].some((name) =>
      name.startsWith('language-'),
    );
    if (!hasLanguage) {
      continue;
    }
    if (code.dataset.ghrmHighlighted === '1') {
      continue;
    }
    window.hljs.highlightElement(code);
    normalizeShellHighlight(code);
    code.dataset.ghrmHighlighted = '1';
  }
}

function highlightBlobCode(code) {
  if (code.dataset.ghrmHighlighted === '1') {
    return;
  }

  const hasLanguage = [...code.classList].some((name) =>
    name.startsWith('language-'),
  );
  if (!hasLanguage || typeof window.hljs?.highlightElement !== 'function') {
    return;
  }

  window.hljs.highlightElement(code);
  normalizeShellHighlight(code);
  code.dataset.ghrmHighlighted = '1';
}

function openTag(node) {
  const attrs = [...node.attributes]
    .map((attr) => `${attr.name}="${escapeHtml(attr.value)}"`)
    .join(' ');
  return attrs
    ? `<${node.tagName.toLowerCase()} ${attrs}>`
    : `<${node.tagName.toLowerCase()}>`;
}

function pushHighlightedNode(node, lines, stack) {
  if (node.nodeType === Node.TEXT_NODE) {
    const parts = node.textContent.split('\n');
    for (let idx = 0; idx < parts.length; idx += 1) {
      if (idx > 0) {
        for (let rev = stack.length - 1; rev >= 0; rev -= 1) {
          lines[lines.length - 1] += `</${stack[rev].tagName.toLowerCase()}>`;
        }
        lines.push('');
        for (const el of stack) {
          lines[lines.length - 1] += openTag(el);
        }
      }
      lines[lines.length - 1] += escapeHtml(parts[idx]);
    }
    return;
  }

  if (node.nodeType !== Node.ELEMENT_NODE) {
    return;
  }

  lines[lines.length - 1] += openTag(node);
  stack.push(node);
  for (const child of node.childNodes) {
    pushHighlightedNode(child, lines, stack);
  }
  stack.pop();
  lines[lines.length - 1] += `</${node.tagName.toLowerCase()}>`;
}

function renderBlob(block) {
  const code = block.querySelector('.ghrm-blob-source code');
  const body = block.querySelector('.ghrm-blob-table tbody');
  if (!code || !body) {
    return;
  }

  highlightBlobCode(code);

  const lines = [''];
  for (const child of code.childNodes) {
    pushHighlightedNode(child, lines, []);
  }

  body.innerHTML = lines
    .map((line, idx) => {
      const content = line || '&#8203;';
      const lineNo = idx + 1;
      return `<tr><td class="ghrm-blob-line-no" data-line-number="${lineNo}"><span class="ghrm-blob-line-no-text">${lineNo}</span></td><td class="ghrm-blob-line-code"><code class="ghrm-blob-line-text">${content}</code></td></tr>`;
    })
    .join('');
}

function renderBlobs() {
  for (const block of document.querySelectorAll('.ghrm-blob')) {
    renderBlob(block);
  }
}

function isShellCode(code) {
  return [...code.classList].some((name) =>
    ['language-bash', 'language-sh', 'language-shell'].includes(name),
  );
}

function normalizeShellHighlight(code) {
  if (!isShellCode(code)) {
    return;
  }

  for (const node of code.querySelectorAll('.hljs-built_in')) {
    if (SHELL_BUILTINS.has(node.textContent.trim())) {
      continue;
    }
    node.replaceWith(document.createTextNode(node.textContent));
  }
}

function mermaidControls() {
  return `<div class="ghrm-mermaid-controls">
    <button type="button" data-action="up" aria-label="Pan up">${icon('chevron-up')}</button>
    <button type="button" data-action="zoom-in" aria-label="Zoom in">${icon('zoom-in')}</button>
    <button type="button" data-action="left" aria-label="Pan left">${icon('chevron-left')}</button>
    <button type="button" data-action="reset" aria-label="Reset">${icon('reset')}</button>
    <button type="button" data-action="right" aria-label="Pan right">${icon('chevron-right')}</button>
    <button type="button" data-action="down" aria-label="Pan down">${icon('chevron-down')}</button>
    <button type="button" data-action="zoom-out" aria-label="Zoom out">${icon('zoom-out')}</button>
  </div>`;
}

function setupMermaidControls(block, target) {
  const svg = target.querySelector('svg');
  if (!svg || typeof window.svgPanZoom !== 'function') {
    return;
  }

  if (block._ghrmPanZoom) {
    block._ghrmPanZoom.destroy();
    block._ghrmPanZoom = null;
  }

  const existing = target.querySelector('.ghrm-mermaid-controls');
  if (existing) existing.remove();

  target.insertAdjacentHTML('beforeend', mermaidControls());

  const naturalHeight = svg.getBoundingClientRect().height;
  const containerHeight = Math.max(naturalHeight, 200);
  target.style.height = `${containerHeight}px`;

  svg.removeAttribute('width');
  svg.removeAttribute('height');
  svg.style.width = '100%';
  svg.style.height = '100%';
  svg.style.overflow = 'visible';

  const panZoom = window.svgPanZoom(svg, {
    center: true,
    contain: false,
    controlIconsEnabled: false,
    dblClickZoomEnabled: false,
    fit: true,
    maxZoom: 10,
    minZoom: 0.5,
    mouseWheelZoomEnabled: false,
    panEnabled: false,
    zoomEnabled: true,
    zoomScaleSensitivity: 0.3,
  });

  panZoom.resize();
  panZoom.fit();
  panZoom.center();

  const step = 50;
  for (const btn of target.querySelectorAll('.ghrm-mermaid-controls button')) {
    btn.addEventListener('click', () => {
      const action = btn.dataset.action;
      if (action === 'zoom-in') panZoom.zoomIn();
      else if (action === 'zoom-out') panZoom.zoomOut();
      else if (action === 'reset') {
        panZoom.resetZoom();
        panZoom.center();
      } else {
        const pan = panZoom.getPan();
        if (action === 'up') panZoom.pan({ x: pan.x, y: pan.y + step });
        else if (action === 'down') panZoom.pan({ x: pan.x, y: pan.y - step });
        else if (action === 'left') panZoom.pan({ x: pan.x + step, y: pan.y });
        else if (action === 'right') panZoom.pan({ x: pan.x - step, y: pan.y });
      }
    });
  }

  block._ghrmPanZoom = panZoom;
}

function ensureMermaidActions(block) {
  let actions = block.querySelector(':scope > .ghrm-render-actions');
  if (actions) {
    return actions;
  }

  actions = document.createElement('div');
  actions.className = 'ghrm-render-actions';

  const fullscreen = document.createElement('button');
  fullscreen.type = 'button';
  fullscreen.className = 'ghrm-action-button';
  fullscreen.setAttribute('aria-label', 'Open fullscreen view');
  fullscreen.innerHTML = fullscreenIcon();
  fullscreen.addEventListener('click', async () => {
    if (document.fullscreenElement === block) {
      await document.exitFullscreen();
      return;
    }

    if (typeof block.requestFullscreen === 'function') {
      await block.requestFullscreen();
    }
  });

  const copy = document.createElement('button');
  copy.type = 'button';
  copy.className = 'ghrm-action-button ghrm-action-copy';
  copy.setAttribute('aria-label', 'Copy mermaid code');
  copy.dataset.copyLabel = 'Copy mermaid code';
  copy.dataset.copyFeedback = 'Copied!';
  copy.innerHTML = `${copyIcon()}${checkIcon()}`;
  copy.addEventListener('click', async () => {
    await writeClipboard(getSource(block));
    showCopied(copy);
  });

  actions.append(fullscreen, copy);
  block.prepend(actions);
  return actions;
}

async function renderMermaid() {
  if (!hasFeature('mermaid')) return;

  const blocks = document.querySelectorAll('.ghrm-mermaid');
  if (blocks.length === 0) return;

  const api = window.mermaid;
  if (!api) return;

  api.initialize({
    startOnLoad: false,
    ...mermaidTheme(),
  });

  for (const block of blocks) {
    const source = getSource(block);
    const target = block.querySelector('.ghrm-mermaid-diagram');
    if (!source || !target) {
      continue;
    }

    clearError(block);
    target.innerHTML = '';

    try {
      if (source.trim() === 'info') {
        const version = await getMermaidVersion(api);
        target.innerHTML = `<pre class="ghrm-mermaid-info">mermaid ${version}</pre>`;
        const actions = block.querySelector(':scope > .ghrm-render-actions');
        if (actions) {
          actions.hidden = true;
        }
        continue;
      }

      const result = await api.render(`ghrm-mermaid-${mermaidId++}`, source);
      target.innerHTML = result.svg;
      if (typeof result.bindFunctions === 'function') {
        result.bindFunctions(target);
      }
      ensureMermaidActions(block).hidden = false;
      setupMermaidControls(block, target);
    } catch (error) {
      setError(block, error.message);
    }
  }
}

async function getMermaidVersion(api) {
  if (typeof api.version === 'function') {
    return api.version();
  }

  if (api.version) {
    return api.version;
  }

  if (!mermaidVersionPromise) {
    const versionPath = assetPlan().mermaidVersion;
    if (!versionPath) return 'unknown';
    mermaidVersionPromise = fetch(versionPath)
      .then((r) => r.text())
      .then((t) => t.trim() || 'unknown')
      .catch(() => 'unknown');
  }

  return mermaidVersionPromise;
}

function topojsonToGeojson(data) {
  const objects = Object.values(data.objects || {});
  const features = [];

  for (const object of objects) {
    const value = window.topojson.feature(data, object);
    if (value.type === 'FeatureCollection') {
      features.push(...value.features);
    } else {
      features.push(value);
    }
  }

  return {
    type: 'FeatureCollection',
    features,
  };
}

function renderMapBlock(block, kind) {
  if (block._ghrmMap) {
    block._ghrmMap.remove();
    block._ghrmMap = null;
  }

  const previous = block.querySelector('.ghrm-map-canvas');
  const canvas = previous.cloneNode(false);
  previous.replaceWith(canvas);

  const source = getSource(block);
  if (!source) {
    return;
  }

  const data = JSON.parse(source);
  const geojson = kind === 'topojson' ? topojsonToGeojson(data) : data;
  const colors = themeColors();
  const map = window.L.map(canvas, {
    attributionControl: false,
    zoomControl: true,
    scrollWheelZoom: true,
  });

  if (navigator.onLine) {
    window.L.tileLayer('https://tile.openstreetmap.org/{z}/{x}/{y}.png', {
      maxZoom: 19,
    }).addTo(map);
  } else {
    const offline = document.createElement('div');
    offline.className = 'ghrm-map-offline';
    offline.innerHTML = icon('cloud-offline');
    canvas.appendChild(offline);
  }

  const layer = window.L.geoJSON(geojson, {
    style(feature) {
      const type = feature?.geometry?.type || '';
      if (type === 'Point' || type === 'MultiPoint') {
        return { color: colors.point, weight: 2 };
      }
      if (type.includes('Line')) {
        return { color: colors.line, weight: 3, opacity: 1 };
      }
      return {
        color: colors.polygon,
        fillColor: colors.polygonFill,
        fillOpacity: 0.3,
        opacity: 0.8,
        weight: 2,
      };
    },
    pointToLayer(_feature, latlng) {
      return window.L.circleMarker(latlng, {
        color: colors.point,
        fillColor: colors.point,
        fillOpacity: 0.9,
        radius: 6,
        weight: 1,
      });
    },
  }).addTo(map);

  const bounds = layer.getBounds();
  if (bounds.isValid()) {
    map.fitBounds(bounds.pad(0.1));
  } else {
    map.setView([0, 0], 1);
  }

  block._ghrmMap = map;
}

async function renderMaps() {
  if (!hasFeature('map')) return;

  const geojsonBlocks = document.querySelectorAll('.ghrm-geojson');
  const topojsonBlocks = document.querySelectorAll('.ghrm-topojson');
  if (geojsonBlocks.length === 0 && topojsonBlocks.length === 0) return;

  if (!window.L) return;

  for (const block of geojsonBlocks) {
    clearError(block);
    try {
      renderMapBlock(block, 'geojson');
    } catch (error) {
      setError(block, error.message);
    }
  }
  for (const block of topojsonBlocks) {
    clearError(block);
    try {
      renderMapBlock(block, 'topojson');
    } catch (error) {
      setError(block, error.message);
    }
  }
}

async function runAll() {
  await loadAssets();
  renderCode();
  renderBlobs();
  await renderMath();
  await renderMermaid();
  await renderMaps();
  addCopyButtons();
}

document.addEventListener('DOMContentLoaded', runAll);
document.addEventListener('ghrm:contentready', runAll);

document.addEventListener('ghrm:themechange', async () => {
  await loadAssets();
  await renderMermaid();
  await renderMaps();
  addCopyButtons();
});

let mermaidId = 0;
let mermaidVersionPromise;
const copyResetDelay = 1000;
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

export function copyIcon() {
  return `
    <svg aria-hidden="true" height="16" viewBox="0 0 16 16" width="16" class="ghrm-copy-icon ghrm-copy-icon-copy">
      <path d="M0 6.75C0 5.784.784 5 1.75 5h1.5a.75.75 0 0 1 0 1.5h-1.5a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h7.5a.25.25 0 0 0 .25-.25v-1.5a.75.75 0 0 1 1.5 0v1.5A1.75 1.75 0 0 1 9.25 16h-7.5A1.75 1.75 0 0 1 0 14.25Z"></path>
      <path d="M5 1.75C5 .784 5.784 0 6.75 0h7.5C15.216 0 16 .784 16 1.75v7.5A1.75 1.75 0 0 1 14.25 11h-7.5A1.75 1.75 0 0 1 5 9.25Zm1.75-.25a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h7.5a.25.25 0 0 0 .25-.25v-7.5a.25.25 0 0 0-.25-.25Z"></path>
    </svg>
  `;
}

export function checkIcon() {
  return `
    <svg aria-hidden="true" height="16" viewBox="0 0 16 16" width="16" class="ghrm-copy-icon ghrm-copy-icon-check">
      <path d="M13.78 4.22a.75.75 0 0 1 0 1.06l-7.25 7.25a.75.75 0 0 1-1.06 0L2.22 9.28a.751.751 0 0 1 .018-1.042.751.751 0 0 1 1.042-.018L6 10.94l6.72-6.72a.75.75 0 0 1 1.06 0Z"></path>
    </svg>
  `;
}

function fullscreenIcon() {
  return `
    <svg aria-hidden="true" height="16" viewBox="0 0 16 16" width="16" class="ghrm-action-icon">
      <path d="M3.72 3.72a.75.75 0 0 1 1.06 1.06L2.56 7h10.88l-2.22-2.22a.75.75 0 0 1 1.06-1.06l3.5 3.5a.75.75 0 0 1 0 1.06l-3.5 3.5a.75.75 0 1 1-1.06-1.06l2.22-2.22H2.56l2.22 2.22a.75.75 0 1 1-1.06 1.06l-3.5-3.5a.75.75 0 0 1 0-1.06Z"></path>
    </svg>
  `;
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
        primaryColor: '#161b22',
        primaryBorderColor: '#8b949e',
        primaryTextColor: '#e6edf3',
        lineColor: '#c9d1d9',
        secondaryColor: '#161b22',
        tertiaryColor: '#161b22',
        mainBkg: '#0d1117',
        nodeBkg: '#161b22',
        clusterBkg: '#0d1117',
        clusterBorder: '#30363d',
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

function renderMath() {
  if (typeof window.renderMathInElement !== 'function') {
    return;
  }

  for (const container of document.querySelectorAll('.markdown-body')) {
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

function escapeHtml(text) {
  return text
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;');
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

function mermaidNavIcon(action) {
  const icons = {
    'zoom-in':
      '<path d="M3.75 7.5a.75.75 0 0 1 .75-.75h2.25V4.5a.75.75 0 0 1 1.5 0v2.25h2.25a.75.75 0 0 1 0 1.5H8.25v2.25a.75.75 0 0 1-1.5 0V8.25H4.5a.75.75 0 0 1-.75-.75Z"></path><path d="M7.5 0a7.5 7.5 0 0 1 5.807 12.247l2.473 2.473a.749.749 0 1 1-1.06 1.06l-2.473-2.473A7.5 7.5 0 1 1 7.5 0Zm-6 7.5a6 6 0 1 0 12 0 6 6 0 0 0-12 0Z"></path>',
    'zoom-out':
      '<path d="M4.5 6.75h6a.75.75 0 0 1 0 1.5h-6a.75.75 0 0 1 0-1.5Z"></path><path d="M0 7.5a7.5 7.5 0 1 1 13.307 4.747l2.473 2.473a.749.749 0 1 1-1.06 1.06l-2.473-2.473A7.5 7.5 0 0 1 0 7.5Zm7.5-6a6 6 0 1 0 0 12 6 6 0 0 0 0-12Z"></path>',
    reset:
      '<path d="M1.705 8.005a.75.75 0 0 1 .834.656 5.5 5.5 0 0 0 9.592 2.97l-1.204-1.204a.25.25 0 0 1 .177-.427h3.646a.25.25 0 0 1 .25.25v3.646a.25.25 0 0 1-.427.177l-1.38-1.38A7.002 7.002 0 0 1 1.05 8.84a.75.75 0 0 1 .656-.834ZM8 2.5a5.487 5.487 0 0 0-4.131 1.869l1.204 1.204A.25.25 0 0 1 4.896 6H1.25A.25.25 0 0 1 1 5.75V2.104a.25.25 0 0 1 .427-.177l1.38 1.38A7.002 7.002 0 0 1 14.95 7.16a.75.75 0 0 1-1.49.178A5.5 5.5 0 0 0 8 2.5Z"></path>',
    up: '<path d="M3.22 10.53a.749.749 0 0 1 0-1.06l4.25-4.25a.749.749 0 0 1 1.06 0l4.25 4.25a.749.749 0 1 1-1.06 1.06L8 6.811 4.28 10.53a.749.749 0 0 1-1.06 0Z"></path>',
    down: '<path d="M12.78 5.22a.749.749 0 0 1 0 1.06l-4.25 4.25a.749.749 0 0 1-1.06 0L3.22 6.28a.749.749 0 1 1 1.06-1.06L8 8.939l3.72-3.719a.749.749 0 0 1 1.06 0Z"></path>',
    left: '<path d="M9.78 12.78a.75.75 0 0 1-1.06 0L4.47 8.53a.75.75 0 0 1 0-1.06l4.25-4.25a.751.751 0 0 1 1.042.018.751.751 0 0 1 .018 1.042L6.06 8l3.72 3.72a.75.75 0 0 1 0 1.06Z"></path>',
    right:
      '<path d="M6.22 3.22a.75.75 0 0 1 1.06 0l4.25 4.25a.75.75 0 0 1 0 1.06l-4.25 4.25a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042L9.94 8 6.22 4.28a.75.75 0 0 1 0-1.06Z"></path>',
  };
  return icons[action] || '';
}

function mermaidNavButton(action, label, classes = '') {
  const className = ['ghrm-mermaid-button', classes].filter(Boolean).join(' ');
  return `<button type="button" class="${className}" data-action="${action}" aria-label="${label}">
    <svg aria-hidden="true" height="16" viewBox="0 0 16 16" width="16" class="ghrm-action-icon">
      ${mermaidNavIcon(action)}
    </svg>
  </button>`;
}

function mermaidNav() {
  return `
    <div class="ghrm-mermaid-nav" aria-label="Mermaid navigation">
      ${mermaidNavButton('up', 'Pan up', 'ghrm-mermaid-up')}
      ${mermaidNavButton('zoom-in', 'Zoom in', 'ghrm-mermaid-zoom-in')}
      ${mermaidNavButton('left', 'Pan left', 'ghrm-mermaid-left')}
      ${mermaidNavButton('reset', 'Reset view', 'ghrm-mermaid-reset')}
      ${mermaidNavButton('right', 'Pan right', 'ghrm-mermaid-right')}
      ${mermaidNavButton('down', 'Pan down', 'ghrm-mermaid-down')}
      ${mermaidNavButton('zoom-out', 'Zoom out', 'ghrm-mermaid-zoom-out')}
    </div>
  `;
}

function destroyMermaidNav(block) {
  if (block._ghrmMermaid?.panZoom) {
    block._ghrmMermaid.panZoom.destroy();
  }

  block._ghrmMermaid = null;
}

function handleMermaidNav(panZoom, action) {
  if (action === 'reset') {
    panZoom.resetZoom();
    panZoom.center();
    return;
  }

  if (action === 'zoom-in') {
    panZoom.zoomIn();
    return;
  }

  if (action === 'zoom-out') {
    panZoom.zoomOut();
    return;
  }

  const step = 48;
  const pan = panZoom.getPan();
  if (action === 'up') {
    panZoom.pan({ x: pan.x, y: pan.y + step });
  } else if (action === 'right') {
    panZoom.pan({ x: pan.x - step, y: pan.y });
  } else if (action === 'down') {
    panZoom.pan({ x: pan.x, y: pan.y - step });
  } else if (action === 'left') {
    panZoom.pan({ x: pan.x + step, y: pan.y });
  }
}

function setupMermaidNav(block, target) {
  const svg = target.querySelector('svg');
  if (!svg || typeof window.svgPanZoom !== 'function') {
    return;
  }

  destroyMermaidNav(block);
  target.classList.add('ghrm-mermaid-interactive');
  target.insertAdjacentHTML('beforeend', mermaidNav());
  svg.removeAttribute('width');
  svg.removeAttribute('height');
  svg.setAttribute('width', '100%');
  svg.setAttribute('height', '100%');
  svg.setAttribute('preserveAspectRatio', 'xMidYMid meet');

  const panZoom = window.svgPanZoom(svg, {
    center: true,
    controlIconsEnabled: false,
    dblClickZoomEnabled: false,
    fit: true,
    maxZoom: 25,
    minZoom: 0.5,
    mouseWheelZoomEnabled: true,
    panEnabled: true,
    zoomEnabled: true,
    zoomScaleSensitivity: 0.3,
  });
  panZoom.resize();
  panZoom.fit();
  panZoom.center();

  for (const button of target.querySelectorAll('.ghrm-mermaid-button')) {
    button.addEventListener('click', () => {
      handleMermaidNav(panZoom, button.dataset.action || '');
    });
  }

  block._ghrmMermaid = { panZoom };
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
  const api = window.mermaid;
  const blocks = document.querySelectorAll('.ghrm-mermaid');
  if (!api || blocks.length === 0) {
    return;
  }

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

    destroyMermaidNav(block);
    clearError(block);
    target.innerHTML = '';
    target.classList.remove('ghrm-mermaid-interactive');

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
      setupMermaidNav(block, target);
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
    mermaidVersionPromise = fetch('/vendor/mermaid-version.txt')
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

  window.L.tileLayer('https://tile.openstreetmap.org/{z}/{x}/{y}.png', {
    maxZoom: 19,
  }).addTo(map);

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

function renderMaps() {
  if (!window.L) {
    return;
  }

  for (const [selector, kind] of [
    ['.ghrm-geojson', 'geojson'],
    ['.ghrm-topojson', 'topojson'],
  ]) {
    for (const block of document.querySelectorAll(selector)) {
      clearError(block);
      try {
        renderMapBlock(block, kind);
      } catch (error) {
        setError(block, error.message);
      }
    }
  }
}

async function runAll() {
  renderCode();
  renderBlobs();
  renderMath();
  await renderMermaid();
  renderMaps();
  addCopyButtons();
}

document.addEventListener('DOMContentLoaded', runAll);
document.addEventListener('ghrm:contentready', runAll);

document.addEventListener('ghrm:themechange', async () => {
  await renderMermaid();
  renderMaps();
  addCopyButtons();
});

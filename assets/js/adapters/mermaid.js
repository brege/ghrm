import { icon } from '../dom.js';
import { assetPlan, hasFeature } from '../vendor.js';
import { clearError, getSource, isDarkTheme, setError } from './common.js';
import { checkIcon, copyIcon, showCopied, writeClipboard } from './copy.js';

let mermaidId = 0;
let mermaidVersionPromise;

function fullscreenIcon() {
  return icon('fullscreen', 'ghrm-action-icon');
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

function mermaidControls() {
  return `<div class="ghrm-mermaid-controls">
    <button type="button" data-action="up" aria-label="Pan up">${icon('chevron-up', 'ghrm-action-icon')}</button>
    <button type="button" data-action="zoom-in" aria-label="Zoom in">${icon('zoom-in', 'ghrm-action-icon')}</button>
    <button type="button" data-action="left" aria-label="Pan left">${icon('chevron-left', 'ghrm-action-icon')}</button>
    <button type="button" data-action="reset" aria-label="Reset">${icon('reset', 'ghrm-action-icon')}</button>
    <button type="button" data-action="right" aria-label="Pan right">${icon('chevron-right', 'ghrm-action-icon')}</button>
    <button type="button" data-action="down" aria-label="Pan down">${icon('chevron-down', 'ghrm-action-icon')}</button>
    <button type="button" data-action="zoom-out" aria-label="Zoom out">${icon('zoom-out', 'ghrm-action-icon')}</button>
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

export async function renderMermaid() {
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

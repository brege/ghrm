import { renderBlobs, renderCode } from './adapters/code.js';
import { addCopyButtons } from './adapters/copy.js';
import { renderMaps } from './adapters/map.js';
import { renderMath } from './adapters/math.js';
import { renderMermaid } from './adapters/mermaid.js';
import { loadAssets } from './vendor.js';

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

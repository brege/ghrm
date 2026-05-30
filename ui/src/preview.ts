import { renderBlobs, renderCode } from './adapters/code';
import { addCopyButtons } from './adapters/copy';
import { renderMaps } from './adapters/map';
import { renderMath } from './adapters/math';
import { renderMermaid } from './adapters/mermaid';
import { syncPrintMode } from './prefs';
import { loadAssets } from './vendor';

async function runAll(): Promise<void> {
  syncPrintMode();
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

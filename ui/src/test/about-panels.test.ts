import { beforeEach, describe, expect, it } from 'vitest';
import { applyAboutPanelPrefs, toggleAboutPanel } from '../status';

function renderAboutPanels(): void {
  document.body.innerHTML = `
    <section id="ghrm-about-peek">
      <div id="ghrm-about-panel-menu">
        <button class="ghrm-view-option is-active" data-ghrm-about-panel-option="scope" aria-checked="true"></button>
        <button class="ghrm-view-option is-active" data-ghrm-about-panel-option="paths" aria-checked="true"></button>
        <button class="ghrm-view-option is-active" data-ghrm-about-panel-option="network" aria-checked="true"></button>
        <button class="ghrm-view-option is-active" data-ghrm-about-panel-option="filters" aria-checked="true"></button>
      </div>
      <div class="ghrm-about-details">
        <section data-ghrm-about-panel="scope"></section>
        <div class="ghrm-detail-grid">
          <section data-ghrm-about-panel="paths"></section>
          <section data-ghrm-about-panel="network"></section>
        </div>
      </div>
    </section>
  `;
}

function must<T extends HTMLElement>(selector: string): T {
  const el = document.querySelector<T>(selector);
  if (!el) {
    throw new Error(`missing ${selector}`);
  }
  return el;
}

describe('about detail panel chooser', () => {
  beforeEach(() => {
    localStorage.clear();
    renderAboutPanels();
  });

  it('toggles panel visibility and option check state', () => {
    toggleAboutPanel('network');

    const panel = must<HTMLElement>('[data-ghrm-about-panel="network"]');
    const option = must<HTMLElement>(
      '[data-ghrm-about-panel-option="network"]',
    );
    expect(panel.hidden).toBe(true);
    expect(option.getAttribute('aria-checked')).toBe('false');
    expect(option.classList.contains('is-active')).toBe(false);

    toggleAboutPanel('network');

    expect(panel.hidden).toBe(false);
    expect(option.getAttribute('aria-checked')).toBe('true');
    expect(option.classList.contains('is-active')).toBe(true);
  });

  it('hides grid wrappers when every child panel is hidden', () => {
    toggleAboutPanel('paths');
    toggleAboutPanel('network');

    const grid = must<HTMLElement>('.ghrm-detail-grid');
    expect(grid.hidden).toBe(true);
  });

  it('hides menu options for panels missing from the current view', () => {
    document.querySelector('[data-ghrm-about-panel="scope"]')?.remove();

    applyAboutPanelPrefs();

    const option = must<HTMLElement>('[data-ghrm-about-panel-option="scope"]');
    expect(option.hidden).toBe(true);
  });
});

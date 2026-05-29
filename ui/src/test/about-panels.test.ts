import { beforeEach, describe, expect, it } from 'vitest';
import { applyAboutPanelPrefs, toggleAboutPanel } from '../status';

function renderAboutPanels(): void {
  document.body.innerHTML = `
    <section id="ghrm-about-peek">
      <div id="ghrm-about-panel-menu">
        <div class="ghrm-about-menu-section" role="group">
          <button class="ghrm-view-option is-active" data-ghrm-about-panel-option="about" aria-checked="true"></button>
          <button class="ghrm-view-option is-active" data-ghrm-about-panel-option="history" aria-checked="true"></button>
        </div>
        <div class="ghrm-about-menu-section" role="group">
          <button class="ghrm-view-option is-active" data-ghrm-about-panel-option="directory" aria-checked="true"></button>
          <button class="ghrm-view-option is-active" data-ghrm-about-panel-option="paths" aria-checked="true"></button>
          <button class="ghrm-view-option is-active" data-ghrm-about-panel-option="network" aria-checked="true"></button>
          <button class="ghrm-view-option is-active" data-ghrm-about-panel-option="filters" aria-checked="true"></button>
        </div>
        <div class="ghrm-about-menu-section" role="group">
          <a class="ghrm-about-menu-link" href="/ghrm">ghrm</a>
        </div>
      </div>
      <div class="ghrm-about-panel-grid">
        <section data-ghrm-about-panel="about"></section>
        <section data-ghrm-about-panel="directory"></section>
        <section data-ghrm-about-panel="filters"></section>
        <section data-ghrm-about-panel="paths"></section>
        <section data-ghrm-about-panel="network"></section>
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

describe('about panel chooser', () => {
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
    toggleAboutPanel('about');
    toggleAboutPanel('directory');
    toggleAboutPanel('filters');
    toggleAboutPanel('paths');
    toggleAboutPanel('network');

    const grid = must<HTMLElement>('.ghrm-about-panel-grid');
    expect(grid.hidden).toBe(true);
  });

  it('hides menu options for panels missing from the current view', () => {
    document.querySelector('[data-ghrm-about-panel="directory"]')?.remove();

    applyAboutPanelPrefs();

    const option = must<HTMLElement>(
      '[data-ghrm-about-panel-option="directory"]',
    );
    expect(option.hidden).toBe(true);
  });

  it('hides empty chooser sections', () => {
    document.querySelector('[data-ghrm-about-panel="about"]')?.remove();
    document.querySelector('[data-ghrm-about-panel="history"]')?.remove();

    applyAboutPanelPrefs();

    const section = must<HTMLElement>('.ghrm-about-menu-section');
    expect(section.hidden).toBe(true);
  });
});

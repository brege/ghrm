import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '../../../islands/explorer/menu';
import type { GhrmExplorerMenus } from '../../../islands/explorer/menu';

function createMenuFixture(): void {
  document.body.innerHTML = `
    <div class="ghrm-header-actions">
      <div class="ghrm-menu-shell">
        <button id="ghrm-view-menu-toggle" aria-expanded="false">Filter</button>
        <div id="ghrm-view-menu" class="ghrm-view-menu" hidden>
          <a class="ghrm-view-option" href="/filter">Option 1</a>
        </div>
      </div>
      <div class="ghrm-menu-shell">
        <button id="ghrm-sort-menu-toggle" aria-expanded="false">Sort</button>
        <div id="ghrm-sort-menu" class="ghrm-view-menu" hidden>
          <a class="ghrm-view-option" href="/sort">Sort A</a>
        </div>
      </div>
      <a id="ghrm-sort-dir-toggle" href="/sort-dir">Dir</a>
      <div class="ghrm-menu-shell">
        <button id="ghrm-archive-menu-toggle" aria-expanded="false">Archive</button>
        <div id="ghrm-archive-menu" class="ghrm-view-menu" hidden>
          <button class="ghrm-view-option" data-ghrm-archive-url="/_ghrm/archive/test">Download</button>
        </div>
      </div>
      <div class="ghrm-menu-shell">
        <button id="ghrm-column-menu-toggle" aria-expanded="false">Columns</button>
        <div id="ghrm-column-menu" class="ghrm-view-menu" hidden>
          <a class="ghrm-view-option" href="/columns">Col A</a>
        </div>
      </div>
    </div>
    <ghrm-explorer-menus></ghrm-explorer-menus>
    <ghrm-archive-progress></ghrm-archive-progress>
  `;
}

describe('ghrm-explorer-menus', () => {
  let element: GhrmExplorerMenus;

  beforeEach(async () => {
    createMenuFixture();
    const found = document.querySelector<GhrmExplorerMenus>(
      'ghrm-explorer-menus',
    );
    if (!found) throw new Error('missing ghrm-explorer-menus');
    element = found;
    await element.updateComplete;
  });

  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  it('opens menu on toggle click', () => {
    const toggle = document.getElementById('ghrm-view-menu-toggle')!;
    const panel = document.getElementById('ghrm-view-menu')!;

    toggle.click();

    expect(panel.hidden).toBe(false);
    expect(toggle.getAttribute('aria-expanded')).toBe('true');
  });

  it('closes menu on second toggle click', () => {
    const toggle = document.getElementById('ghrm-view-menu-toggle')!;
    const panel = document.getElementById('ghrm-view-menu')!;

    toggle.click();
    expect(panel.hidden).toBe(false);

    toggle.click();
    expect(panel.hidden).toBe(true);
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
  });

  it('closes other menus when opening a new one', () => {
    const filterToggle = document.getElementById('ghrm-view-menu-toggle')!;
    const filterPanel = document.getElementById('ghrm-view-menu')!;
    const sortToggle = document.getElementById('ghrm-sort-menu-toggle')!;
    const sortPanel = document.getElementById('ghrm-sort-menu')!;

    filterToggle.click();
    expect(filterPanel.hidden).toBe(false);
    expect(sortPanel.hidden).toBe(true);

    sortToggle.click();
    expect(filterPanel.hidden).toBe(true);
    expect(sortPanel.hidden).toBe(false);
  });

  it('closes menus on outside click', () => {
    const toggle = document.getElementById('ghrm-view-menu-toggle')!;
    const panel = document.getElementById('ghrm-view-menu')!;

    toggle.click();
    expect(panel.hidden).toBe(false);

    document.body.click();
    expect(panel.hidden).toBe(true);
  });

  it('does not close menu when clicking inside panel', () => {
    const toggle = document.getElementById('ghrm-view-menu-toggle')!;
    const panel = document.getElementById('ghrm-view-menu')!;

    toggle.click();
    expect(panel.hidden).toBe(false);

    panel.click();
    expect(panel.hidden).toBe(false);
  });

  it('does not close menu when clicking sort dir toggle', () => {
    const filterToggle = document.getElementById('ghrm-view-menu-toggle')!;
    const filterPanel = document.getElementById('ghrm-view-menu')!;
    const dirToggle = document.getElementById('ghrm-sort-dir-toggle')!;

    filterToggle.click();
    expect(filterPanel.hidden).toBe(false);

    dirToggle.click();
    expect(filterPanel.hidden).toBe(false);
  });

  it('closes menu on Escape and returns focus to toggle', () => {
    const toggle = document.getElementById('ghrm-view-menu-toggle')!;
    const panel = document.getElementById('ghrm-view-menu')!;

    toggle.click();
    expect(panel.hidden).toBe(false);

    const event = new KeyboardEvent('keydown', {
      key: 'Escape',
      bubbles: true,
    });
    document.dispatchEvent(event);

    expect(panel.hidden).toBe(true);
    expect(document.activeElement).toBe(toggle);
  });

  it('repositions panel on resize', () => {
    const toggle = document.getElementById('ghrm-view-menu-toggle')!;
    const panel = document.getElementById('ghrm-view-menu')!;

    toggle.click();
    expect(panel.hidden).toBe(false);

    vi.spyOn(toggle, 'getBoundingClientRect').mockReturnValue({
      bottom: 100,
      right: 200,
      top: 80,
      left: 160,
      width: 40,
      height: 20,
      x: 160,
      y: 80,
      toJSON: () => {},
    });

    window.dispatchEvent(new Event('resize'));

    expect(panel.style.top).toBe('108px');
  });

  it('cleans up listeners on disconnect', () => {
    const docRemoveSpy = vi.spyOn(document, 'removeEventListener');
    const winRemoveSpy = vi.spyOn(window, 'removeEventListener');

    element.remove();

    const docCalls = docRemoveSpy.mock.calls.map(([event]) => event);
    const winCalls = winRemoveSpy.mock.calls.map(([event]) => event);

    expect(docCalls).toContain('click');
    expect(docCalls).toContain('keydown');
    expect(winCalls).toContain('resize');
  });

  it('does not duplicate listeners on reconnect', () => {
    const addClickSpy = vi.spyOn(document, 'addEventListener');

    element.remove();
    document.body.appendChild(element);

    const clickCalls = addClickSpy.mock.calls.filter(
      ([event]) => event === 'click',
    );
    expect(clickCalls.length).toBe(1);
  });

  it('triggers archive progress on archive option click', async () => {
    await import('../../../islands/archive/progress');

    const archiveToggle = document.getElementById('ghrm-archive-menu-toggle')!;
    const archivePanel = document.getElementById('ghrm-archive-menu')!;
    const archiveOption = archivePanel.querySelector(
      '[data-ghrm-archive-url]',
    ) as HTMLElement;

    const progressElement = document.querySelector('ghrm-archive-progress');
    const startJobSpy = vi.fn();
    if (progressElement) {
      (progressElement as any).startJob = startJobSpy;
    }

    archiveToggle.click();
    expect(archivePanel.hidden).toBe(false);

    const clickEvent = new MouseEvent('click', { bubbles: true });
    archiveOption.dispatchEvent(clickEvent);

    expect(archivePanel.hidden).toBe(true);
    expect(startJobSpy).toHaveBeenCalledWith('/_ghrm/archive/test');
  });

  it('handles missing menus gracefully', () => {
    document.body.innerHTML = '<ghrm-explorer-menus></ghrm-explorer-menus>';

    expect(() => {
      document.body.click();
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
      window.dispatchEvent(new Event('resize'));
    }).not.toThrow();
  });
});

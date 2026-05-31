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

  describe('menu toggle behavior', () => {
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
      const columnToggle = document.getElementById('ghrm-column-menu-toggle')!;
      const columnPanel = document.getElementById('ghrm-column-menu')!;

      filterToggle.click();
      expect(filterPanel.hidden).toBe(false);
      expect(columnPanel.hidden).toBe(true);

      columnToggle.click();
      expect(filterPanel.hidden).toBe(true);
      expect(columnPanel.hidden).toBe(false);
    });
  });

  describe('outside click behavior', () => {
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
  });

  describe('keyboard behavior', () => {
    it('closes menu on Escape and returns focus to toggle', () => {
      const toggle = document.getElementById('ghrm-view-menu-toggle')!;
      const panel = document.getElementById('ghrm-view-menu')!;

      toggle.click();
      expect(panel.hidden).toBe(false);

      document.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }),
      );

      expect(panel.hidden).toBe(true);
      expect(document.activeElement).toBe(toggle);
    });
  });

  describe('resize behavior', () => {
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
  });

  describe('option click behavior', () => {
    it('link option closes menu without preventing default', () => {
      const toggle = document.getElementById('ghrm-view-menu-toggle')!;
      const panel = document.getElementById('ghrm-view-menu')!;
      const option = panel.querySelector('.ghrm-view-option') as HTMLElement;

      toggle.click();
      expect(panel.hidden).toBe(false);

      const clickEvent = new MouseEvent('click', {
        bubbles: true,
        cancelable: true,
      });
      const preventDefaultSpy = vi.spyOn(clickEvent, 'preventDefault');
      option.dispatchEvent(clickEvent);

      expect(panel.hidden).toBe(true);
      expect(preventDefaultSpy).not.toHaveBeenCalled();
    });

    it('archive option prevents default and starts archive progress', async () => {
      await import('../../../islands/archive/progress');

      const archiveToggle = document.getElementById(
        'ghrm-archive-menu-toggle',
      )!;
      const archivePanel = document.getElementById('ghrm-archive-menu')!;
      const archiveOption = archivePanel.querySelector(
        '[data-ghrm-archive-url]',
      ) as HTMLElement;

      const progressElement = document.querySelector('ghrm-archive-progress');
      const startJobSpy = vi.fn();
      if (progressElement) {
        (
          progressElement as unknown as { startJob: typeof startJobSpy }
        ).startJob = startJobSpy;
      }

      archiveToggle.click();
      expect(archivePanel.hidden).toBe(false);

      const clickEvent = new MouseEvent('click', {
        bubbles: true,
        cancelable: true,
      });
      const preventDefaultSpy = vi.spyOn(clickEvent, 'preventDefault');
      archiveOption.dispatchEvent(clickEvent);

      expect(archivePanel.hidden).toBe(true);
      expect(preventDefaultSpy).toHaveBeenCalled();
      expect(startJobSpy).toHaveBeenCalledWith('/_ghrm/archive/test');
    });
  });

  describe('lifecycle behavior', () => {
    it('global listeners do not fire after disconnect', () => {
      const toggle = document.getElementById('ghrm-view-menu-toggle')!;
      const panel = document.getElementById('ghrm-view-menu')!;

      toggle.click();
      expect(panel.hidden).toBe(false);

      element.remove();

      document.body.click();
      expect(panel.hidden).toBe(false);

      document.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }),
      );
      expect(panel.hidden).toBe(false);
    });

    it('global listeners fire after reconnect', async () => {
      element.remove();
      document.body.appendChild(element);
      await element.updateComplete;

      const toggle = document.getElementById('ghrm-view-menu-toggle')!;
      const panel = document.getElementById('ghrm-view-menu')!;

      toggle.click();
      expect(panel.hidden).toBe(false);

      document.body.click();
      expect(panel.hidden).toBe(true);
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

  describe('htmx-style DOM replacement', () => {
    it('newly inserted island upgrades and works after article replacement', async () => {
      document.body.innerHTML = '';

      document.body.innerHTML = `
        <article>
          <div class="ghrm-header-actions">
            <div class="ghrm-menu-shell">
              <button id="ghrm-view-menu-toggle" aria-expanded="false">Filter</button>
              <div id="ghrm-view-menu" class="ghrm-view-menu" hidden>
                <a class="ghrm-view-option" href="/filter">Option 1</a>
              </div>
            </div>
            <div class="ghrm-menu-shell">
              <button id="ghrm-archive-menu-toggle" aria-expanded="false">Archive</button>
              <div id="ghrm-archive-menu" class="ghrm-view-menu" hidden></div>
            </div>
            <div class="ghrm-menu-shell">
              <button id="ghrm-column-menu-toggle" aria-expanded="false">Columns</button>
              <div id="ghrm-column-menu" class="ghrm-view-menu" hidden></div>
            </div>
          </div>
          <ghrm-explorer-menus></ghrm-explorer-menus>
        </article>
      `;

      const newElement = document.querySelector<GhrmExplorerMenus>(
        'ghrm-explorer-menus',
      );
      expect(newElement).toBeTruthy();
      await newElement!.updateComplete;

      const toggle = document.getElementById('ghrm-view-menu-toggle')!;
      const panel = document.getElementById('ghrm-view-menu')!;

      toggle.click();
      expect(panel.hidden).toBe(false);
      expect(toggle.getAttribute('aria-expanded')).toBe('true');

      document.body.click();
      expect(panel.hidden).toBe(true);
    });
  });
});

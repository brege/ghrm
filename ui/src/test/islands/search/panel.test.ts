// biome-ignore-all lint/style/noNonNullAssertion: test assertions
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '../../../islands/search/panel';
import type { GhrmSearchPanel } from '../../../islands/search/panel';
import {
  hasActiveSearch,
  refreshActiveSearch,
  resetSearchState,
  setSearchCloseHandler,
} from '../../../islands/search/panel';

function createSearchFixture(withExplorer = true): void {
  document.body.innerHTML = `
    ${
      withExplorer
        ? `
    <article data-explorer data-current-path="/test">
      <div class="ghrm-nav-empty">Empty</div>
      <table class="ghrm-nav-table">
        <tbody>
          <tr><td class="ghrm-nav-name"><a href="/file.txt">file.txt</a></td></tr>
        </tbody>
      </table>
    </article>
    `
        : ''
    }
    <div id="ghrm-path-search" class="ghrm-path-search" role="search" hidden>
      <div class="ghrm-search-field">
        <button id="ghrm-search-mode" type="button" title="Switch search mode" aria-label="Switch search mode"></button>
        <input id="ghrm-path-search-input" type="search" placeholder="Search paths" aria-label="Search paths">
      </div>
      <button id="ghrm-path-search-toggle" type="button" aria-expanded="false" aria-label="Search paths"></button>
      <span id="ghrm-path-search-status" aria-live="polite"></span>
      <ghrm-search-panel></ghrm-search-panel>
    </div>
  `;
}

describe('ghrm-search-panel', () => {
  let element: GhrmSearchPanel;

  beforeEach(async () => {
    resetSearchState();
    vi.stubGlobal(
      'fetch',
      vi.fn(() =>
        Promise.resolve({
          ok: true,
          text: () => Promise.resolve('<tr><td>result</td></tr>'),
          headers: new Map([
            ['X-Ghrm-Search-Count', '1'],
            ['X-Ghrm-Search-Truncated', '0'],
            ['X-Ghrm-Search-Pending', '0'],
            ['X-Ghrm-Search-Max-Rows', '100'],
          ]),
        }),
      ),
    );

    createSearchFixture();
    const found = document.querySelector<GhrmSearchPanel>('ghrm-search-panel');
    if (!found) throw new Error('missing ghrm-search-panel');
    element = found;
    await element.updateComplete;
  });

  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  describe('panel visibility', () => {
    it('shows search panel when explorer article exists', () => {
      const search = document.getElementById('ghrm-path-search')!;
      expect(search.hidden).toBe(false);
    });

    it('hides search panel when no explorer article', async () => {
      createSearchFixture(false);
      const newElement =
        document.querySelector<GhrmSearchPanel>('ghrm-search-panel');
      await newElement!.updateComplete;

      const search = document.getElementById('ghrm-path-search')!;
      expect(search.hidden).toBe(true);
    });
  });

  describe('toggle behavior', () => {
    it('opens panel on toggle click', () => {
      const toggle = document.getElementById('ghrm-path-search-toggle')!;
      const search = document.getElementById('ghrm-path-search')!;
      const input = document.getElementById(
        'ghrm-path-search-input',
      ) as HTMLInputElement;

      toggle.click();

      expect(search.classList.contains('is-open')).toBe(true);
      expect(toggle.getAttribute('aria-expanded')).toBe('true');
      expect(input.tabIndex).toBe(0);
    });

    it('closes panel on second toggle click', () => {
      const toggle = document.getElementById('ghrm-path-search-toggle')!;
      const search = document.getElementById('ghrm-path-search')!;

      toggle.click();
      expect(search.classList.contains('is-open')).toBe(true);

      toggle.click();
      expect(search.classList.contains('is-open')).toBe(false);
      expect(toggle.getAttribute('aria-expanded')).toBe('false');
    });

    it('focuses input when opening', () => {
      const toggle = document.getElementById('ghrm-path-search-toggle')!;
      const input = document.getElementById(
        'ghrm-path-search-input',
      ) as HTMLInputElement;

      toggle.click();

      expect(document.activeElement).toBe(input);
    });
  });

  describe('keyboard behavior', () => {
    it('closes on Escape and returns focus to toggle', () => {
      const toggle = document.getElementById('ghrm-path-search-toggle')!;
      const search = document.getElementById('ghrm-path-search')!;
      const input = document.getElementById(
        'ghrm-path-search-input',
      ) as HTMLInputElement;

      toggle.click();
      expect(search.classList.contains('is-open')).toBe(true);

      input.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }),
      );

      expect(search.classList.contains('is-open')).toBe(false);
      expect(document.activeElement).toBe(toggle);
    });
  });

  describe('close behavior', () => {
    it('clears input value on close', () => {
      const toggle = document.getElementById('ghrm-path-search-toggle')!;
      const input = document.getElementById(
        'ghrm-path-search-input',
      ) as HTMLInputElement;

      toggle.click();
      input.value = 'test query';
      toggle.click();

      expect(input.value).toBe('');
    });

    it('calls close handler when dirty', () => {
      const closeHandler = vi.fn();
      setSearchCloseHandler(closeHandler);

      const toggle = document.getElementById('ghrm-path-search-toggle')!;
      const input = document.getElementById(
        'ghrm-path-search-input',
      ) as HTMLInputElement;

      toggle.click();
      input.value = 'test';
      input.dispatchEvent(new Event('input', { bubbles: true }));

      refreshActiveSearch();
      toggle.click();

      expect(closeHandler).toHaveBeenCalled();
    });

    it('restores original rows when not dirty', async () => {
      const tbody = document.querySelector('.ghrm-nav-table tbody')!;
      const originalContent = tbody.innerHTML;

      const toggle = document.getElementById('ghrm-path-search-toggle')!;
      const input = document.getElementById(
        'ghrm-path-search-input',
      ) as HTMLInputElement;

      toggle.click();
      input.value = 'test';
      input.dispatchEvent(new Event('input', { bubbles: true }));

      await new Promise((resolve) => setTimeout(resolve, 50));

      toggle.click();

      expect(tbody.innerHTML).toBe(originalContent);
    });
  });

  describe('search mode', () => {
    it('starts in path mode', () => {
      const search = document.getElementById('ghrm-path-search')!;
      const input = document.getElementById(
        'ghrm-path-search-input',
      ) as HTMLInputElement;

      expect(search.dataset.mode).toBe('path');
      expect(input.placeholder).toBe('Search paths');
    });

    it('switches to content mode on mode button click', () => {
      const toggle = document.getElementById('ghrm-path-search-toggle')!;
      const modeBtn = document.getElementById('ghrm-search-mode')!;
      const search = document.getElementById('ghrm-path-search')!;
      const input = document.getElementById(
        'ghrm-path-search-input',
      ) as HTMLInputElement;

      toggle.click();
      modeBtn.click();

      expect(search.dataset.mode).toBe('content');
      expect(input.placeholder).toBe('Search content');
    });

    it('toggles back to path mode', () => {
      const toggle = document.getElementById('ghrm-path-search-toggle')!;
      const modeBtn = document.getElementById('ghrm-search-mode')!;
      const search = document.getElementById('ghrm-path-search')!;

      toggle.click();
      modeBtn.click();
      modeBtn.click();

      expect(search.dataset.mode).toBe('path');
    });
  });

  describe('stale response handling', () => {
    it('discards stale responses', async () => {
      let resolveFirst: (value: unknown) => void;
      const firstPromise = new Promise((r) => {
        resolveFirst = r;
      });

      const fetchMock = vi
        .fn()
        .mockImplementationOnce(
          () =>
            new Promise((resolve) => {
              resolveFirst = () =>
                resolve({
                  ok: true,
                  text: () => Promise.resolve('<tr><td>stale</td></tr>'),
                  headers: new Map([['X-Ghrm-Search-Count', '1']]),
                });
            }),
        )
        .mockImplementationOnce(() =>
          Promise.resolve({
            ok: true,
            text: () => Promise.resolve('<tr><td>fresh</td></tr>'),
            headers: new Map([['X-Ghrm-Search-Count', '2']]),
          }),
        );

      vi.stubGlobal('fetch', fetchMock);

      const toggle = document.getElementById('ghrm-path-search-toggle')!;
      const input = document.getElementById(
        'ghrm-path-search-input',
      ) as HTMLInputElement;
      const tbody = document.querySelector('.ghrm-nav-table tbody')!;

      toggle.click();

      input.value = 'first';
      input.dispatchEvent(new Event('input', { bubbles: true }));

      input.value = 'second';
      input.dispatchEvent(new Event('input', { bubbles: true }));

      await new Promise((resolve) => setTimeout(resolve, 50));

      resolveFirst!(null);
      await new Promise((resolve) => setTimeout(resolve, 50));

      expect(tbody.innerHTML).toContain('fresh');
      expect(tbody.innerHTML).not.toContain('stale');
    });
  });

  describe('fragment replacement', () => {
    it('replaces tbody content with search results', async () => {
      const toggle = document.getElementById('ghrm-path-search-toggle')!;
      const input = document.getElementById(
        'ghrm-path-search-input',
      ) as HTMLInputElement;
      const tbody = document.querySelector('.ghrm-nav-table tbody')!;

      toggle.click();
      input.value = 'test';
      input.dispatchEvent(new Event('input', { bubbles: true }));

      await new Promise((resolve) => setTimeout(resolve, 50));

      expect(tbody.innerHTML).toContain('result');
    });

    it('shows table when results arrive', async () => {
      const table = document.querySelector('.ghrm-nav-table') as HTMLElement;
      const toggle = document.getElementById('ghrm-path-search-toggle')!;
      const input = document.getElementById(
        'ghrm-path-search-input',
      ) as HTMLInputElement;

      toggle.click();
      input.value = 'test';
      input.dispatchEvent(new Event('input', { bubbles: true }));

      await new Promise((resolve) => setTimeout(resolve, 50));

      expect(table.hidden).toBe(false);
    });

    it('hides empty placeholder when results arrive', async () => {
      const empty = document.querySelector('.ghrm-nav-empty') as HTMLElement;
      const toggle = document.getElementById('ghrm-path-search-toggle')!;
      const input = document.getElementById(
        'ghrm-path-search-input',
      ) as HTMLInputElement;

      toggle.click();
      input.value = 'test';
      input.dispatchEvent(new Event('input', { bubbles: true }));

      await new Promise((resolve) => setTimeout(resolve, 50));

      expect(empty.hidden).toBe(true);
    });
  });

  describe('status updates', () => {
    it('shows result count for path search', async () => {
      const toggle = document.getElementById('ghrm-path-search-toggle')!;
      const input = document.getElementById(
        'ghrm-path-search-input',
      ) as HTMLInputElement;
      const status = document.getElementById('ghrm-path-search-status')!;

      toggle.click();
      input.value = 'test';
      input.dispatchEvent(new Event('input', { bubbles: true }));

      await new Promise((resolve) => setTimeout(resolve, 50));

      expect(status.textContent).toBe('1 path');
    });

    it('clears status on close', async () => {
      const toggle = document.getElementById('ghrm-path-search-toggle')!;
      const input = document.getElementById(
        'ghrm-path-search-input',
      ) as HTMLInputElement;
      const status = document.getElementById('ghrm-path-search-status')!;

      toggle.click();
      input.value = 'test';
      input.dispatchEvent(new Event('input', { bubbles: true }));

      await new Promise((resolve) => setTimeout(resolve, 50));
      expect(status.textContent).toBe('1 path');

      toggle.click();
      expect(status.textContent).toBe('');
    });
  });

  describe('module exports', () => {
    it('hasActiveSearch returns false when closed', () => {
      expect(hasActiveSearch()).toBe(false);
    });

    it('hasActiveSearch returns true when open with query', async () => {
      const toggle = document.getElementById('ghrm-path-search-toggle')!;
      const input = document.getElementById(
        'ghrm-path-search-input',
      ) as HTMLInputElement;

      toggle.click();
      input.value = 'test';
      input.dispatchEvent(new Event('input', { bubbles: true }));

      await new Promise((resolve) => setTimeout(resolve, 50));

      expect(hasActiveSearch()).toBe(true);
    });

    it('refreshActiveSearch returns false when no active search', () => {
      expect(refreshActiveSearch()).toBe(false);
    });
  });

  describe('lifecycle behavior', () => {
    it('cleans up handlers on disconnect', () => {
      const input = document.getElementById(
        'ghrm-path-search-input',
      ) as HTMLInputElement;
      const toggle = document.getElementById('ghrm-path-search-toggle')!;

      element.remove();

      expect(input.oninput).toBeNull();
      expect(input.onkeydown).toBeNull();
      expect(toggle.onclick).toBeNull();
    });

    it('handles reconnection', async () => {
      element.remove();
      document.body.querySelector('#ghrm-path-search')!.appendChild(element);
      await element.updateComplete;

      const toggle = document.getElementById('ghrm-path-search-toggle')!;
      const search = document.getElementById('ghrm-path-search')!;

      toggle.click();
      expect(search.classList.contains('is-open')).toBe(true);
    });
  });

  describe('htmx-style DOM replacement', () => {
    it('newly inserted island upgrades and works after article replacement', async () => {
      document.body.innerHTML = '';

      document.body.innerHTML = `
        <article data-explorer data-current-path="/new">
          <table class="ghrm-nav-table">
            <tbody><tr><td>new content</td></tr></tbody>
          </table>
        </article>
        <div id="ghrm-path-search" class="ghrm-path-search" role="search">
          <button id="ghrm-search-mode" type="button"></button>
          <input id="ghrm-path-search-input" type="search">
          <button id="ghrm-path-search-toggle" type="button" aria-expanded="false"></button>
          <span id="ghrm-path-search-status"></span>
          <ghrm-search-panel></ghrm-search-panel>
        </div>
      `;

      const newElement =
        document.querySelector<GhrmSearchPanel>('ghrm-search-panel');
      expect(newElement).toBeTruthy();
      await newElement!.updateComplete;

      const toggle = document.getElementById('ghrm-path-search-toggle')!;
      const search = document.getElementById('ghrm-path-search')!;

      toggle.click();
      expect(search.classList.contains('is-open')).toBe(true);
      expect(toggle.getAttribute('aria-expanded')).toBe('true');
    });

    it('rebinds to new article on ghrm:contentready after htmx swap', async () => {
      const fetchMock = vi.fn((url: string) => {
        const pathMatch = url.match(/path=([^&]*)/);
        const path = pathMatch ? decodeURIComponent(pathMatch[1]) : '';
        return Promise.resolve({
          ok: true,
          text: () => Promise.resolve(`<tr><td>result for ${path}</td></tr>`),
          headers: new Map([
            ['X-Ghrm-Search-Count', '1'],
            ['X-Ghrm-Search-Truncated', '0'],
            ['X-Ghrm-Search-Pending', '0'],
            ['X-Ghrm-Search-Max-Rows', '100'],
          ]),
        });
      });
      vi.stubGlobal('fetch', fetchMock);

      const toggle = document.getElementById('ghrm-path-search-toggle')!;
      const input = document.getElementById(
        'ghrm-path-search-input',
      ) as HTMLInputElement;
      const oldTbody = document.querySelector('.ghrm-nav-table tbody')!;

      toggle.click();
      input.value = 'query';
      input.dispatchEvent(new Event('input', { bubbles: true }));
      await new Promise((resolve) => setTimeout(resolve, 50));

      expect(oldTbody.innerHTML).toContain('result for /test');

      const oldArticle = document.querySelector('article')!;
      const newArticle = document.createElement('article');
      newArticle.dataset.explorer = '';
      newArticle.dataset.currentPath = '/newpath';
      newArticle.innerHTML = `
        <div class="ghrm-nav-empty">Empty</div>
        <table class="ghrm-nav-table">
          <tbody><tr><td>new original</td></tr></tbody>
        </table>
      `;
      oldArticle.replaceWith(newArticle);

      document.dispatchEvent(new CustomEvent('ghrm:contentready'));
      await element.updateComplete;

      const newTbody = document.querySelector('.ghrm-nav-table tbody')!;
      expect(newTbody).not.toBe(oldTbody);

      input.value = 'another';
      input.dispatchEvent(new Event('input', { bubbles: true }));
      await new Promise((resolve) => setTimeout(resolve, 50));

      expect(newTbody.innerHTML).toContain('result for /newpath');
      expect(oldTbody.innerHTML).toContain('result for /test');
    });
  });
});

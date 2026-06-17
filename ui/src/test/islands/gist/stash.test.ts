import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '../../../islands/gist/stash';
import type { GhrmGistStash } from '../../../islands/gist/stash';

interface StashRow {
  id: string;
  name: string;
  href: string;
  ts?: number;
  size?: number;
  lines?: number;
}

function createStashArticle(rows: StashRow[] = []): string {
  const defaultRows: StashRow[] =
    rows.length > 0
      ? rows
      : [
          {
            id: 'paste-abc',
            name: 'test-paste',
            href: '/_ghrm/gist?id=paste-abc',
          },
        ];
  const rowsHtml = defaultRows
    .map(
      (r) => `
      <tr data-ghrm-gist-row data-ghrm-gist-id="${r.id}">
        <td class="ghrm-gist-name-cell">
          <a href="${r.href}" data-ghrm-gist-row-link>${r.name}</a>
          <button data-ghrm-gist-rename-start>Rename</button>
        </td>
        <td data-column-key="date"${r.ts ? ` data-ts="${r.ts}" data-sort-value="${r.ts}"` : ''}></td>
        <td data-column-key="size" data-sort-value="${r.size ?? 0}">${r.size ?? 0}</td>
        <td data-column-key="lines" data-sort-value="${r.lines ?? 0}">${r.lines ?? 0}</td>
      </tr>
    `,
    )
    .join('');

  return `
    <article data-ghrm-gist-stash>
      <ghrm-gist-stash>
        <table class="ghrm-nav-table">
          <thead>
            <tr>
              <th><button class="ghrm-column-sort" data-ghrm-gist-sort="name">Name<svg class="ghrm-column-sort-icon" hidden><use></use></svg></button></th>
              <th><button class="ghrm-column-sort is-active" data-ghrm-gist-sort="date">Modified<svg class="ghrm-column-sort-icon"><use href="/_ghrm/assets/js/icons.svg#ghrm-icon-chevron-down"></use></svg></button></th>
              <th><button class="ghrm-column-sort" data-ghrm-gist-sort="size">Size<svg class="ghrm-column-sort-icon" hidden><use></use></svg></button></th>
              <th><button class="ghrm-column-sort" data-ghrm-gist-sort="lines">Lines<svg class="ghrm-column-sort-icon" hidden><use></use></svg></button></th>
            </tr>
          </thead>
          <tbody>${rowsHtml}</tbody>
        </table>
      </ghrm-gist-stash>
    </article>
  `;
}

function createStashElement(): GhrmGistStash {
  const template = document.createElement('template');
  template.innerHTML = createStashArticle();
  document.body.appendChild(template.content.cloneNode(true));
  const element = document.querySelector<GhrmGistStash>('ghrm-gist-stash');
  if (!element) throw new Error('missing ghrm-gist-stash');
  return element;
}

describe('ghrm-gist-stash', () => {
  let element: GhrmGistStash;

  beforeEach(async () => {
    element = createStashElement();
    await element.updateComplete;
  });

  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  describe('rename input creation', () => {
    it('clicking rename button creates input element', async () => {
      const renameBtn = element.querySelector<HTMLButtonElement>(
        '[data-ghrm-gist-rename-start]',
      )!;
      expect(renameBtn).toBeTruthy();
      expect(renameBtn.dataset.ghrmBound).toBe('1');

      renameBtn.click();

      const input = element.querySelector<HTMLInputElement>(
        '[data-ghrm-gist-row-input]',
      );
      expect(input).toBeTruthy();
      expect(input?.value).toBe('test-paste');
    });

    it('rename input hides link and button', () => {
      const renameBtn = element.querySelector<HTMLButtonElement>(
        '[data-ghrm-gist-rename-start]',
      )!;
      const link = element.querySelector<HTMLAnchorElement>(
        '[data-ghrm-gist-row-link]',
      )!;

      renameBtn.click();

      expect(link.hidden).toBe(true);
      expect(renameBtn.hidden).toBe(true);
    });

    it('Escape key restores row display without saving', () => {
      const fetchSpy = vi.spyOn(globalThis, 'fetch');
      const renameBtn = element.querySelector<HTMLButtonElement>(
        '[data-ghrm-gist-rename-start]',
      )!;
      const link = element.querySelector<HTMLAnchorElement>(
        '[data-ghrm-gist-row-link]',
      )!;

      renameBtn.click();

      const input = element.querySelector<HTMLInputElement>(
        '[data-ghrm-gist-row-input]',
      )!;
      input.value = 'changed-name';
      input.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }),
      );

      expect(element.querySelector('[data-ghrm-gist-row-input]')).toBeNull();
      expect(link.hidden).toBe(false);
      expect(renameBtn.hidden).toBe(false);
      expect(link.textContent).toBe('test-paste');
      expect(fetchSpy).not.toHaveBeenCalled();
    });
  });

  describe('successful rename', () => {
    it('POST updates row id, href, and visible name', async () => {
      let fetchPromise: Promise<Response> | null = null;
      const fetchSpy = vi.spyOn(globalThis, 'fetch').mockImplementation(() => {
        fetchPromise = Promise.resolve(
          new Response(
            JSON.stringify({
              id: 'paste-xyz',
              href: '/_ghrm/gist?id=paste-xyz',
              name: 'renamed-paste',
            }),
            { status: 200 },
          ),
        );
        return fetchPromise;
      });

      const renameBtn = element.querySelector<HTMLButtonElement>(
        '[data-ghrm-gist-rename-start]',
      )!;
      renameBtn.click();

      const input = element.querySelector<HTMLInputElement>(
        '[data-ghrm-gist-row-input]',
      )!;
      input.value = 'renamed-paste';
      input.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }),
      );

      await vi.waitFor(() => fetchSpy.mock.calls.length > 0);
      if (fetchPromise) await fetchPromise;
      await new Promise((r) => setTimeout(r, 0));

      const row = element.querySelector<HTMLElement>('[data-ghrm-gist-row]')!;
      const link = element.querySelector<HTMLAnchorElement>(
        '[data-ghrm-gist-row-link]',
      )!;

      expect(row.dataset.ghrmGistId).toBe('paste-xyz');
      expect(link.href).toContain('/_ghrm/gist?id=paste-xyz');
      expect(link.textContent).toBe('renamed-paste');
      expect(link.hidden).toBe(false);

      expect(fetchSpy).toHaveBeenCalledWith(
        '/_ghrm/gist/rename/paste-abc',
        expect.objectContaining({
          method: 'POST',
          headers: {
            Accept: 'application/json',
            'Content-Type': 'text/plain; charset=utf-8',
          },
          body: 'renamed-paste',
        }),
      );
    });

    it('restores unchanged name without fetch', () => {
      const fetchSpy = vi.spyOn(globalThis, 'fetch');
      const renameBtn = element.querySelector<HTMLButtonElement>(
        '[data-ghrm-gist-rename-start]',
      )!;

      renameBtn.click();

      const input = element.querySelector<HTMLInputElement>(
        '[data-ghrm-gist-row-input]',
      )!;
      input.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }),
      );

      expect(element.querySelector('[data-ghrm-gist-row-input]')).toBeNull();
      expect(fetchSpy).not.toHaveBeenCalled();
    });
  });

  describe('failed or invalid rename', () => {
    it('invalid name marks input as invalid without hiding', () => {
      const fetchSpy = vi.spyOn(globalThis, 'fetch');
      const renameBtn = element.querySelector<HTMLButtonElement>(
        '[data-ghrm-gist-rename-start]',
      )!;

      renameBtn.click();

      const input = element.querySelector<HTMLInputElement>(
        '[data-ghrm-gist-row-input]',
      )!;
      input.value = '.invalid-name';
      input.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }),
      );

      expect(element.querySelector('[data-ghrm-gist-row-input]')).toBeTruthy();
      expect(input.getAttribute('aria-invalid')).toBe('true');
      expect(input.title).toBe(
        'Use letters, numbers, dots, dashes, or underscores',
      );
      expect(fetchSpy).not.toHaveBeenCalled();
    });

    it('server error marks input as invalid without hiding', async () => {
      vi.spyOn(globalThis, 'fetch').mockResolvedValue(
        new Response('', { status: 409 }),
      );

      const renameBtn = element.querySelector<HTMLButtonElement>(
        '[data-ghrm-gist-rename-start]',
      )!;
      renameBtn.click();

      const input = element.querySelector<HTMLInputElement>(
        '[data-ghrm-gist-row-input]',
      )!;
      input.value = 'conflict-name';
      input.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }),
      );

      await vi.waitFor(() => input.getAttribute('aria-invalid') === 'true');

      expect(element.querySelector('[data-ghrm-gist-row-input]')).toBeTruthy();
      expect(input.title).toBe('Name already exists or is invalid');
    });

    it('failed rename allows retry', async () => {
      let secondFetchPromise: Promise<Response> | null = null;
      const fetchSpy = vi
        .spyOn(globalThis, 'fetch')
        .mockImplementationOnce(() =>
          Promise.resolve(new Response('', { status: 409 })),
        )
        .mockImplementationOnce(() => {
          secondFetchPromise = Promise.resolve(
            new Response(
              JSON.stringify({
                id: 'paste-new',
                href: '/_ghrm/gist?id=paste-new',
                name: 'retry-name',
              }),
              { status: 200 },
            ),
          );
          return secondFetchPromise;
        });

      const renameBtn = element.querySelector<HTMLButtonElement>(
        '[data-ghrm-gist-rename-start]',
      )!;
      renameBtn.click();

      const input = element.querySelector<HTMLInputElement>(
        '[data-ghrm-gist-row-input]',
      )!;
      input.value = 'conflict-name';
      input.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }),
      );

      await vi.waitFor(() => input.getAttribute('aria-invalid') === 'true');

      input.value = 'retry-name';
      input.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }),
      );

      await vi.waitFor(() => fetchSpy.mock.calls.length >= 2);
      if (secondFetchPromise) await secondFetchPromise;
      await new Promise((r) => setTimeout(r, 0));

      const link = element.querySelector<HTMLAnchorElement>(
        '[data-ghrm-gist-row-link]',
      )!;
      expect(link.textContent).toBe('retry-name');
      expect(fetchSpy).toHaveBeenCalledTimes(2);
    });
  });

  describe('lifecycle', () => {
    it('uses a host inside the htmx article boundary', () => {
      const article = document.querySelector('article[data-ghrm-gist-stash]');

      expect(article).toBeTruthy();
      expect(article?.contains(element)).toBe(true);
    });

    it('refreshes to a new upgraded stash host', async () => {
      const oldArticle = document.querySelector(
        'article[data-ghrm-gist-stash]',
      )!;
      const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
        new Response(
          createStashArticle([
            {
              id: 'paste-next',
              name: 'next-paste',
              href: '/_ghrm/gist?id=paste-next',
            },
          ]),
          { status: 200 },
        ),
      );

      document.dispatchEvent(new CustomEvent('ghrm:live:gist'));

      await vi.waitFor(() => fetchSpy.mock.calls.length > 0);
      await vi.waitFor(() => oldArticle.isConnected === false);

      const nextArticle = document.querySelector(
        'article[data-ghrm-gist-stash]',
      )!;
      const nextElement =
        nextArticle.querySelector<GhrmGistStash>('ghrm-gist-stash')!;
      await nextElement.updateComplete;

      expect(nextElement).not.toBe(element);

      const renameBtn = nextElement.querySelector<HTMLButtonElement>(
        '[data-ghrm-gist-rename-start]',
      )!;
      expect(renameBtn.dataset.ghrmBound).toBe('1');

      renameBtn.click();

      const input = nextElement.querySelector<HTMLInputElement>(
        '[data-ghrm-gist-row-input]',
      );
      expect(input?.value).toBe('next-paste');
    });

    it('removes global listeners on disconnect', async () => {
      const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
        new Response('<article data-ghrm-gist-stash></article>', {
          status: 200,
        }),
      );

      element.remove();

      document.dispatchEvent(new CustomEvent('ghrm:live:gist'));
      await Promise.resolve();
      await Promise.resolve();

      expect(fetchSpy).not.toHaveBeenCalled();
    });
  });

  describe('sorting', () => {
    function createSortableStash(): GhrmGistStash {
      document.body.innerHTML = '';
      const rows: StashRow[] = [
        { id: 'b', name: 'beta.txt', href: '/b', ts: 200, size: 50, lines: 5 },
        {
          id: 'a',
          name: 'alpha.txt',
          href: '/a',
          ts: 300,
          size: 100,
          lines: 3,
        },
        {
          id: 'c',
          name: 'gamma.txt',
          href: '/c',
          ts: 100,
          size: 25,
          lines: 10,
        },
      ];
      const template = document.createElement('template');
      template.innerHTML = createStashArticle(rows);
      document.body.appendChild(template.content.cloneNode(true));
      const el = document.querySelector<GhrmGistStash>('ghrm-gist-stash');
      if (!el) throw new Error('missing ghrm-gist-stash');
      return el;
    }

    function getRowIds(): string[] {
      return Array.from(document.querySelectorAll('[data-ghrm-gist-row]')).map(
        (row) => (row as HTMLElement).dataset.ghrmGistId || '',
      );
    }

    it('clicking name header sorts alphabetically ascending', async () => {
      const el = createSortableStash();
      await el.updateComplete;

      const nameHeader = document.querySelector<HTMLElement>(
        '[data-ghrm-gist-sort="name"]',
      );
      nameHeader?.click();

      expect(getRowIds()).toEqual(['a', 'b', 'c']);
      expect(nameHeader?.classList.contains('is-active')).toBe(true);
      expect(nameHeader?.closest('th')?.getAttribute('aria-sort')).toBe(
        'ascending',
      );
    });

    it('clicking date header twice reverses to ascending', async () => {
      const el = createSortableStash();
      await el.updateComplete;

      const dateHeader = document.querySelector<HTMLElement>(
        '[data-ghrm-gist-sort="date"]',
      );
      dateHeader?.click();

      expect(getRowIds()).toEqual(['c', 'b', 'a']);
      expect(dateHeader?.closest('th')?.getAttribute('aria-sort')).toBe(
        'ascending',
      );
    });

    it('clicking size header sorts by size descending', async () => {
      const el = createSortableStash();
      await el.updateComplete;

      const sizeHeader = document.querySelector<HTMLElement>(
        '[data-ghrm-gist-sort="size"]',
      );
      sizeHeader?.click();

      expect(getRowIds()).toEqual(['a', 'b', 'c']);
    });

    it('sorts formatted sizes by raw byte value', async () => {
      const el = createSortableStash();
      await el.updateComplete;
      const sizeCell = document.querySelector<HTMLElement>(
        '[data-ghrm-gist-id="a"] [data-column-key="size"]',
      );
      if (!sizeCell) throw new Error('missing size cell');
      sizeCell.textContent = '1.2 KB';
      sizeCell.dataset.sortValue = '1200';

      const sizeHeader = document.querySelector<HTMLElement>(
        '[data-ghrm-gist-sort="size"]',
      );
      sizeHeader?.click();

      expect(getRowIds()).toEqual(['a', 'b', 'c']);
    });

    it('clicking lines header sorts by lines descending', async () => {
      const el = createSortableStash();
      await el.updateComplete;

      const linesHeader = document.querySelector<HTMLElement>(
        '[data-ghrm-gist-sort="lines"]',
      );
      linesHeader?.click();

      expect(getRowIds()).toEqual(['c', 'b', 'a']);
    });
  });
});

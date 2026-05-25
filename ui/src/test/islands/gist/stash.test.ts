import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '../../../islands/gist/stash';
import type { GhrmGistStash } from '../../../islands/gist/stash';

function createStashArticle(
  rows: Array<{ id: string; name: string; href: string }> = [],
): string {
  const defaultRows =
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
      </tr>
    `,
    )
    .join('');

  return `
    <article data-ghrm-gist-stash>
      <ghrm-gist-stash>
        <table class="ghrm-nav-table">
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
});

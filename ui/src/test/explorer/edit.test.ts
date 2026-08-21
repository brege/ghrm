import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { bindExplorerEdit } from '../../explorer/edit';
import { EDIT_PENDING_KEY } from '../../file/file';

function fixture(currentPath = 'docs'): void {
  document.body.innerHTML = `
    <article data-explorer="true" data-current-path="${currentPath}">
      <div class="ghrm-header-actions">
        <div class="ghrm-expand ghrm-new-file" data-ghrm-new-file>
          <div class="ghrm-expand-field">
            <input class="ghrm-expand-input" data-ghrm-expand-input type="text" aria-label="New file name" tabindex="-1">
          </div>
          <button type="button" data-ghrm-expand-toggle aria-expanded="false">New</button>
        </div>
      </div>
      <table><tbody>
        <tr>
          <td class="ghrm-nav-name ghrm-row-host">
            <a href="/docs/a.md">a.md</a>
            <span class="ghrm-row-seat" data-ghrm-row-path="docs/a.md">
              <button type="button" data-ghrm-row-rename>Rename</button>
              <button type="button" data-ghrm-row-delete>Delete</button>
            </span>
          </td>
        </tr>
      </tbody></table>
    </article>`;
}

function bind(): {
  navigate: ReturnType<typeof vi.fn>;
  refresh: ReturnType<typeof vi.fn>;
} {
  const navigate = vi.fn();
  const refresh = vi.fn().mockResolvedValue(undefined);
  bindExplorerEdit({ navigate, refresh });
  return { navigate, refresh };
}

function openNewFile(): HTMLButtonElement {
  const toggle = document.querySelector<HTMLButtonElement>(
    '[data-ghrm-new-file] [data-ghrm-expand-toggle]',
  )!;
  toggle.click();
  return toggle;
}

function newFileInput(): HTMLInputElement {
  return document.querySelector<HTMLInputElement>(
    '[data-ghrm-new-file] [data-ghrm-expand-input]',
  )!;
}

describe('explorer edit controls', () => {
  beforeEach(() => {
    fixture();
    sessionStorage.clear();
  });

  afterEach(() => {
    document.body.innerHTML = '';
    sessionStorage.clear();
    vi.restoreAllMocks();
  });

  it('refreshes the explorer after deletion', async () => {
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(new Response(null, { status: 204 }));
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    const { refresh } = bind();

    document
      .querySelector<HTMLButtonElement>('[data-ghrm-row-delete]')!
      .click();

    await vi.waitUntil(() => refresh.mock.calls.length > 0);
    const [url, options] = fetchSpy.mock.calls[0];
    expect(url).toBe('/_ghrm/edit/docs/a.md');
    expect(options?.method).toBe('DELETE');
    expect(refresh).toHaveBeenCalledWith(
      document.querySelector('article[data-explorer]'),
    );
  });

  it('keeps the row when confirmation is declined', () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch');
    vi.spyOn(window, 'confirm').mockReturnValue(false);
    bind();

    document
      .querySelector<HTMLButtonElement>('[data-ghrm-row-delete]')!
      .click();

    expect(fetchSpy).not.toHaveBeenCalled();
    expect(document.querySelector('tr')).toBeTruthy();
  });

  it('renames a row through PATCH and refreshes the explorer', async () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          path: 'docs/b.md',
          name: 'b.md',
          href: '/docs/b.md',
        }),
        { status: 200 },
      ),
    );
    const { refresh } = bind();

    document
      .querySelector<HTMLButtonElement>('[data-ghrm-row-rename]')!
      .click();
    const input = document.querySelector<HTMLInputElement>(
      '[data-ghrm-rename-input]',
    )!;
    input.value = 'b.md';
    input.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }),
    );

    await vi.waitUntil(
      () => document.querySelector('[data-ghrm-rename-input]') === null,
    );
    const [url, options] = fetchSpy.mock.calls[0];
    expect(url).toBe('/_ghrm/edit/docs/a.md');
    expect(options?.method).toBe('PATCH');
    expect(options?.body).toBe('b.md');
    expect(refresh).toHaveBeenCalledWith(
      document.querySelector('article[data-explorer]'),
    );
  });

  it('expands the new-file field from its toggle', () => {
    bind();
    const root = document.querySelector<HTMLElement>('[data-ghrm-new-file]')!;
    const toggle = openNewFile();
    const input = newFileInput();

    expect(root.classList.contains('is-open')).toBe(true);
    expect(toggle.getAttribute('aria-expanded')).toBe('true');
    expect(input.tabIndex).toBe(0);

    input.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }),
    );

    expect(root.classList.contains('is-open')).toBe(false);
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    expect(input.tabIndex).toBe(-1);
    expect(input.value).toBe('');
  });

  it('rejects a name carrying a path separator without a request', () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch');
    bind();

    openNewFile();
    const input = newFileInput();
    input.value = 'nested/fresh.md';
    input.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }),
    );

    expect(fetchSpy).not.toHaveBeenCalled();
    expect(input.getAttribute('aria-invalid')).toBe('true');
    expect(input.title).toBe('Use a file name without path separators');
  });

  it('creates a file with If-None-Match and opens it pending edit', async () => {
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(new Response('{}', { status: 201 }));
    const { navigate } = bind();

    openNewFile();
    const input = newFileInput();
    input.value = 'fresh.md';
    input.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }),
    );

    await vi.waitUntil(() => navigate.mock.calls.length > 0);
    const [url, options] = fetchSpy.mock.calls[0];
    expect(url).toBe('/_ghrm/edit/docs/fresh.md');
    expect(options?.method).toBe('PUT');
    expect((options?.headers as Record<string, string>)['If-None-Match']).toBe(
      '*',
    );
    expect(sessionStorage.getItem(EDIT_PENDING_KEY)).toBe('docs/fresh.md');
    expect(navigate).toHaveBeenCalledWith('/docs/fresh.md');
  });

  it('reports an existing file without navigating', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response('', { status: 412 }),
    );
    const { navigate } = bind();

    openNewFile();
    const input = newFileInput();
    input.value = 'taken.md';
    input.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }),
    );

    await vi.waitUntil(() => input.getAttribute('aria-invalid') === 'true');
    expect(input.title).toBe('File already exists');
    expect(navigate).not.toHaveBeenCalled();
    expect(sessionStorage.getItem(EDIT_PENDING_KEY)).toBeNull();
  });

  it('encodes reserved characters when opening a created file', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response('{}', { status: 201 }),
    );
    const { navigate } = bind();

    openNewFile();
    const input = newFileInput();
    input.value = 'notes #1?.md';
    input.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }),
    );

    await vi.waitUntil(() => navigate.mock.calls.length > 0);
    expect(navigate).toHaveBeenCalledWith('/docs/notes%20%231%3F.md');
    expect(sessionStorage.getItem(EDIT_PENDING_KEY)).toBe('docs/notes #1?.md');
  });
});

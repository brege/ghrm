import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '../../../islands/gist/editor';
import * as indent from '../../../indent';
import type { GhrmGistEditor } from '../../../islands/gist/editor';

function createEditorArticle(
  opts: { id?: string; body?: string; name?: string } = {},
): string {
  const id = opts.id ?? 'test-paste-id';
  const body = opts.body ?? 'initial content';
  const name = opts.name ?? id;
  return `
    <article data-ghrm-gist data-ghrm-gist-page="/_ghrm/gist" data-ghrm-gist-id="${id}">
      <ghrm-gist-editor>
        <form data-ghrm-gist-form>
          <div data-ghrm-gist-editor>
            <div class="ghrm-blob">
              <div class="ghrm-blob-source"><code></code></div>
              <table class="ghrm-blob-table"><tbody></tbody></table>
            </div>
            <textarea>${body}</textarea>
          </div>
          <span data-ghrm-gist-save-control>
            <button type="button" data-ghrm-gist-save disabled>Save</button>
          </span>
          <input data-ghrm-gist-name type="text" value="${name}">
          <span data-ghrm-gist-status></span>
        </form>
        <button data-ghrm-gist-wrap>Wrap</button>
        <button data-ghrm-gist-copy>Copy</button>
      </ghrm-gist-editor>
    </article>
  `;
}

function createEditorElement(
  opts: { id?: string; body?: string; name?: string } = {},
): GhrmGistEditor {
  const template = document.createElement('template');
  template.innerHTML = createEditorArticle(opts);
  document.body.appendChild(template.content.cloneNode(true));
  const element = document.querySelector<GhrmGistEditor>('ghrm-gist-editor');
  if (!element) throw new Error('missing ghrm-gist-editor');
  return element;
}

describe('ghrm-gist-editor', () => {
  let element: GhrmGistEditor;

  beforeEach(async () => {
    localStorage.clear();
    element = createEditorElement();
    await element.updateComplete;
    await new Promise((r) => requestAnimationFrame(r));
  });

  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
    localStorage.clear();
  });

  describe('save request', () => {
    it('sends POST request with correct method, headers, and body', async () => {
      const fetchSpy = vi
        .spyOn(globalThis, 'fetch')
        .mockResolvedValue(new Response('', { status: 200 }));

      const textarea = element.querySelector('textarea')!;
      textarea.value = 'new content';
      textarea.dispatchEvent(new Event('input', { bubbles: true }));

      const saveButton = element.querySelector<HTMLButtonElement>(
        '[data-ghrm-gist-save]',
      )!;
      saveButton.click();

      await vi.waitFor(() => fetchSpy.mock.calls.length > 0);

      const [url, options] = fetchSpy.mock.calls[0];
      expect(url).toBe('/_ghrm/gist');
      expect(options?.method).toBe('POST');
      expect((options?.headers as Record<string, string>)?.Accept).toBe(
        'application/json',
      );
      expect(
        (options?.headers as Record<string, string>)?.['Content-Type'],
      ).toBe('text/plain; charset=utf-8');
      expect(options?.body).toBe('new content');
    });

    it('includes X-Ghrm-Gist-Id header when editing existing paste', async () => {
      const fetchSpy = vi
        .spyOn(globalThis, 'fetch')
        .mockResolvedValue(new Response('', { status: 200 }));

      const textarea = element.querySelector('textarea')!;
      textarea.value = 'updated content';
      textarea.dispatchEvent(new Event('input', { bubbles: true }));

      const saveButton = element.querySelector<HTMLButtonElement>(
        '[data-ghrm-gist-save]',
      )!;
      saveButton.click();

      await vi.waitFor(() => fetchSpy.mock.calls.length > 0);

      const headers = fetchSpy.mock.calls[0][1]?.headers as Record<
        string,
        string
      >;
      expect(headers['X-Ghrm-Gist-Id']).toBe('test-paste-id');
    });

    it('includes X-Ghrm-Gist-Name header when name is provided', async () => {
      document.body.innerHTML = '';
      const customElement = createEditorElement({ name: 'custom-name' });
      await customElement.updateComplete;
      await new Promise((r) => requestAnimationFrame(r));

      const fetchSpy = vi
        .spyOn(globalThis, 'fetch')
        .mockResolvedValue(new Response('', { status: 200 }));

      const textarea = customElement.querySelector('textarea')!;
      textarea.value = 'new content';
      textarea.dispatchEvent(new Event('input', { bubbles: true }));

      const saveButton = customElement.querySelector<HTMLButtonElement>(
        '[data-ghrm-gist-save]',
      )!;
      saveButton.click();

      await vi.waitFor(() => fetchSpy.mock.calls.length > 0);

      const headers = fetchSpy.mock.calls[0][1]?.headers as Record<
        string,
        string
      >;
      expect(headers['X-Ghrm-Gist-Name']).toBe('custom-name');
    });
  });

  describe('dirty state and save button', () => {
    it('save button is disabled when content matches saved state', () => {
      const saveButton = element.querySelector<HTMLButtonElement>(
        '[data-ghrm-gist-save]',
      )!;
      expect(saveButton.disabled).toBe(true);
    });

    it('save button is enabled when content differs from saved state', () => {
      const textarea = element.querySelector('textarea')!;
      textarea.value = 'modified content';
      textarea.dispatchEvent(new Event('input', { bubbles: true }));

      const saveButton = element.querySelector<HTMLButtonElement>(
        '[data-ghrm-gist-save]',
      )!;
      expect(saveButton.disabled).toBe(false);
    });

    it('save button is disabled when name is invalid', () => {
      const textarea = element.querySelector('textarea')!;
      textarea.value = 'modified content';
      textarea.dispatchEvent(new Event('input', { bubbles: true }));

      const nameInput = element.querySelector<HTMLInputElement>(
        '[data-ghrm-gist-name]',
      )!;
      nameInput.value = '.invalid';
      nameInput.dispatchEvent(new Event('input', { bubbles: true }));

      const saveButton = element.querySelector<HTMLButtonElement>(
        '[data-ghrm-gist-save]',
      )!;
      expect(saveButton.disabled).toBe(true);
      expect(nameInput.getAttribute('aria-invalid')).toBe('true');
    });

    it('save button shows "Saving" label during save', async () => {
      vi.spyOn(globalThis, 'fetch').mockImplementation(
        () => new Promise(() => {}),
      );

      const textarea = element.querySelector('textarea')!;
      textarea.value = 'new content';
      textarea.dispatchEvent(new Event('input', { bubbles: true }));

      const saveButton = element.querySelector<HTMLButtonElement>(
        '[data-ghrm-gist-save]',
      )!;
      saveButton.click();

      await vi.waitFor(
        () => saveButton.getAttribute('aria-label') === 'Saving',
      );
      expect(saveButton.disabled).toBe(true);
    });
  });

  describe('Tab indentation', () => {
    it('calls indentEdit helper on Tab keydown', () => {
      const indentSpy = vi.spyOn(indent, 'indentEdit').mockReturnValue({
        start: 0,
        end: 0,
        text: '  ',
        selectionStart: 2,
        selectionEnd: 2,
      });
      const textarea = element.querySelector('textarea')!;
      textarea.value = 'line one';
      textarea.selectionStart = 0;
      textarea.selectionEnd = 0;

      const event = new KeyboardEvent('keydown', {
        key: 'Tab',
        bubbles: true,
        cancelable: true,
      });
      textarea.dispatchEvent(event);

      expect(indentSpy).toHaveBeenCalledWith('line one', 0, 0, false);
    });

    it('passes shiftKey to indentEdit for outdent', () => {
      const indentSpy = vi.spyOn(indent, 'indentEdit').mockReturnValue({
        start: 0,
        end: 10,
        text: 'indented',
        selectionStart: 0,
        selectionEnd: 8,
      });
      const textarea = element.querySelector('textarea')!;
      textarea.value = '  indented';
      textarea.selectionStart = 2;
      textarea.selectionEnd = 2;

      textarea.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: 'Tab',
          shiftKey: true,
          bubbles: true,
          cancelable: true,
        }),
      );

      expect(indentSpy).toHaveBeenCalledWith('  indented', 2, 2, true);
    });

    it('ignores Tab with modifier keys', () => {
      const indentSpy = vi.spyOn(indent, 'indentEdit');
      const textarea = element.querySelector('textarea')!;

      textarea.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: 'Tab',
          ctrlKey: true,
          bubbles: true,
        }),
      );
      textarea.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: 'Tab',
          altKey: true,
          bubbles: true,
        }),
      );
      textarea.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: 'Tab',
          metaKey: true,
          bubbles: true,
        }),
      );

      expect(indentSpy).not.toHaveBeenCalled();
    });
  });

  describe('live refresh deferral', () => {
    it('defers refresh when unsaved changes exist', async () => {
      const fetchSpy = vi
        .spyOn(globalThis, 'fetch')
        .mockResolvedValue(
          new Response('<article data-ghrm-gist></article>', { status: 200 }),
        );

      const textarea = element.querySelector('textarea')!;
      textarea.value = 'unsaved changes';
      textarea.dispatchEvent(new Event('input', { bubbles: true }));

      document.dispatchEvent(new CustomEvent('ghrm:live:gist'));
      await Promise.resolve();
      await Promise.resolve();

      expect(fetchSpy).not.toHaveBeenCalled();
    });

    it('refreshes when content is clean', async () => {
      const oldArticle = document.querySelector('article[data-ghrm-gist]')!;
      const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
        new Response(
          createEditorArticle({
            id: 'fresh-paste-id',
            body: 'fresh content',
            name: 'fresh-paste-id',
          }),
          { status: 200 },
        ),
      );

      document.dispatchEvent(new CustomEvent('ghrm:live:gist'));

      await vi.waitFor(() => fetchSpy.mock.calls.length > 0);
      await vi.waitFor(() => oldArticle.isConnected === false);

      const [url, options] = fetchSpy.mock.calls[0];
      expect(url).toBe('/_ghrm/gist');
      expect((options?.headers as Record<string, string>)?.Accept).toBe(
        'text/html',
      );
      expect((options?.headers as Record<string, string>)?.['HX-Request']).toBe(
        'true',
      );

      const nextArticle = document.querySelector<HTMLElement>(
        'article[data-ghrm-gist]',
      )!;
      const nextElement =
        nextArticle.querySelector<GhrmGistEditor>('ghrm-gist-editor')!;
      await nextElement.updateComplete;

      expect(nextArticle.dataset.ghrmGistId).toBe('fresh-paste-id');
      expect(nextElement).not.toBe(element);

      const textarea = nextElement.querySelector('textarea')!;
      const saveButton = nextElement.querySelector<HTMLButtonElement>(
        '[data-ghrm-gist-save]',
      )!;
      textarea.value = 'fresh content with edits';
      textarea.dispatchEvent(new Event('input', { bubbles: true }));

      expect(saveButton.disabled).toBe(false);
    });
  });

  describe('lifecycle', () => {
    it('uses a host inside the htmx article boundary', () => {
      const article = document.querySelector('article[data-ghrm-gist]');

      expect(article).toBeTruthy();
      expect(article?.contains(element)).toBe(true);
    });

    it('removes global listeners on disconnect', async () => {
      const fetchSpy = vi
        .spyOn(globalThis, 'fetch')
        .mockResolvedValue(
          new Response('<article data-ghrm-gist></article>', { status: 200 }),
        );

      element.remove();

      document.dispatchEvent(new CustomEvent('ghrm:live:gist'));
      await Promise.resolve();
      await Promise.resolve();

      expect(fetchSpy).not.toHaveBeenCalled();
    });
  });
});

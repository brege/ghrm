import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FileEditor } from '../../file/file-edit';

interface Harness {
  container: HTMLElement;
  editor: FileEditor;
  editBtn: HTMLButtonElement;
  saveBtn: HTMLButtonElement;
  rawToggle: HTMLButtonElement;
}

function setup(kind = 'source', body = 'seed body'): Harness {
  document.body.innerHTML = `
    <article class="markdown-body">
    <section class="ghrm-page-shell" data-ghrm-view-kind="${kind}" data-current-path="notes.txt" data-ghrm-raw-url="/_ghrm/raw/notes.txt" data-ghrm-edit-version="seed-version">
      <div class="ghrm-page-content">
        <div class="ghrm-file-pane" data-ghrm-preview-pane hidden></div>
        <div class="ghrm-file-pane ghrm-raw-pane" data-ghrm-raw-pane>
          <div class="ghrm-blob">
            <template class="ghrm-data">${body}</template>
            <div class="ghrm-blob-source"><code>${body}</code></div>
            <table class="ghrm-blob-table"><tbody></tbody></table>
          </div>
        </div>
      </div>
    </section>
    </article>`;
  const container = document.querySelector<HTMLElement>('.ghrm-page-shell')!;
  const group = document.createElement('div');
  group.className = 'ghrm-file-edit';
  const editBtn = document.createElement('button');
  const saveBtn = document.createElement('button');
  group.append(editBtn, saveBtn);
  const rawToggle = document.createElement('button');
  const editor = new FileEditor(
    container,
    '/_ghrm/edit/notes.txt',
    editBtn,
    saveBtn,
    rawToggle,
  );
  return { container, editor, editBtn, saveBtn, rawToggle };
}

function savedResponse(version = 'saved-version'): Response {
  return new Response('{}', {
    status: 200,
    headers: { ETag: `"${version}"` },
  });
}

describe('FileEditor', () => {
  let h: Harness;

  beforeEach(() => {
    h = setup();
  });

  afterEach(() => {
    if (h.editor.editing) {
      vi.spyOn(window, 'confirm').mockReturnValue(true);
      h.editor.exit();
    }
    document.body.innerHTML = '';
    window.onpopstate = null;
    vi.restoreAllMocks();
  });

  it('seeds a textarea from the blob data and marks the view editing', () => {
    h.editor.toggle();

    const textarea = h.container.querySelector('textarea');
    expect(textarea).toBeTruthy();
    expect(textarea?.value).toBe('seed body');
    expect(h.container.dataset.ghrmEditing).toBe('1');
    expect(h.rawToggle.disabled).toBe(true);
    expect(h.saveBtn.hidden).toBe(false);
  });

  it('marks the edit control active while editing', () => {
    h.editor.toggle();
    expect(h.editBtn.classList.contains('is-active')).toBe(true);
    expect(h.saveBtn.hidden).toBe(false);

    h.editor.toggle();
    expect(h.editBtn.classList.contains('is-active')).toBe(false);
    expect(h.saveBtn.hidden).toBe(true);
  });

  it('enables save only when the text changes', () => {
    h.editor.toggle();
    const textarea = h.container.querySelector('textarea')!;

    expect(h.saveBtn.disabled).toBe(true);

    textarea.value = 'edited body';
    textarea.dispatchEvent(new Event('input', { bubbles: true }));

    expect(h.saveBtn.disabled).toBe(false);
  });

  it('PUTs the edited text to the edit endpoint', async () => {
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(savedResponse());

    h.editor.toggle();
    const textarea = h.container.querySelector('textarea')!;
    textarea.value = 'edited body';
    textarea.dispatchEvent(new Event('input', { bubbles: true }));

    await h.editor.save();

    const [url, options] = fetchSpy.mock.calls[0];
    expect(url).toBe('/_ghrm/edit/notes.txt');
    expect(options?.method).toBe('PUT');
    expect((options?.headers as Record<string, string>)['Content-Type']).toBe(
      'text/plain; charset=utf-8',
    );
    expect((options?.headers as Record<string, string>)['If-Match']).toBe(
      '"seed-version"',
    );
    expect(options?.body).toBe('edited body');
    expect(h.saveBtn.disabled).toBe(true);
    expect(h.container.dataset.ghrmEditing).toBe('1');
    expect(h.container.dataset.ghrmEditVersion).toBe('saved-version');
  });

  it('does not send a request when nothing changed', async () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch');

    h.editor.toggle();
    await h.editor.save();

    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it('converts to CRLF on save when the source uses CRLF', async () => {
    h = setup('source', 'seed\nbody');
    h.container.dataset.ghrmEol = 'crlf';
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(savedResponse());

    h.editor.toggle();
    const textarea = h.container.querySelector('textarea')!;
    textarea.value = 'first\nsecond';
    textarea.dispatchEvent(new Event('input', { bubbles: true }));

    await h.editor.save();

    expect(fetchSpy.mock.calls[0][1]?.body).toBe('first\r\nsecond');
  });

  it('normalizes edited text to the dominant line ending', async () => {
    h = setup('source', 'first\nsecond\nthird\nfourth');
    h.container.dataset.ghrmEol = 'crlf';
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(savedResponse());

    h.editor.toggle();
    const textarea = h.container.querySelector('textarea')!;
    textarea.value = 'First\nsecond\nthird\nfourth';
    textarea.dispatchEvent(new Event('input', { bubbles: true }));
    await h.editor.save();

    expect(fetchSpy.mock.calls[0][1]?.body).toBe(
      'First\r\nsecond\r\nthird\r\nfourth',
    );

    h.editor.exit();
    h = setup('source', 'first\nsecond');
    fetchSpy.mockClear();
    h.editor.toggle();
    const crTextarea = h.container.querySelector('textarea')!;
    crTextarea.value = 'First\nsecond';
    crTextarea.dispatchEvent(new Event('input', { bubbles: true }));
    await h.editor.save();

    expect(fetchSpy.mock.calls[0][1]?.body).toBe('First\nsecond');
  });

  it('warns before overwriting an externally changed file', async () => {
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(new Response(null, { headers: { ETag: '"disk"' } }));
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(false);

    h.editor.toggle();
    const textarea = h.container.querySelector('textarea')!;
    textarea.value = 'edited body';
    textarea.dispatchEvent(new Event('input', { bubbles: true }));
    h.container.dataset.ghrmEditConflict = '1';

    await h.editor.save();

    expect(confirmSpy).toHaveBeenCalled();
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it('turns a failed precondition into a conflict', async () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response('file changed', {
        status: 412,
        headers: { ETag: '"external-version"' },
      }),
    );

    h.editor.toggle();
    const textarea = h.container.querySelector('textarea')!;
    textarea.value = 'edited body';
    textarea.dispatchEvent(new Event('input', { bubbles: true }));
    await h.editor.save();

    expect(fetchSpy).toHaveBeenCalledOnce();
    expect(h.container.dataset.ghrmEditConflict).toBe('1');
    expect(h.container.dataset.ghrmEditConflictVersion).toBe(
      'external-version',
    );
    expect(h.saveBtn.disabled).toBe(false);
  });

  it('exits cleanly and clears the editing marker', () => {
    // A markdown view has a preview, so its code toggle is re-enabled on exit.
    h = setup('markdown');
    h.editor.toggle();
    expect(h.editor.editing).toBe(true);

    h.editor.toggle();

    expect(h.editor.editing).toBe(false);
    expect(h.container.querySelector('textarea')).toBeNull();
    expect(h.container.dataset.ghrmEditing).toBeUndefined();
    expect(h.rawToggle.disabled).toBe(false);
  });

  it('leaves the disabled code toggle disabled for source files on exit', () => {
    h.rawToggle.disabled = true;
    h.editor.toggle();
    h.editor.toggle();

    expect(h.editor.editing).toBe(false);
    expect(h.rawToggle.disabled).toBe(true);
  });

  it('confirms before discarding unsaved edits', () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(false);

    h.editor.toggle();
    const textarea = h.container.querySelector('textarea')!;
    textarea.value = 'edited body';
    textarea.dispatchEvent(new Event('input', { bubbles: true }));

    h.editor.toggle();

    expect(confirmSpy).toHaveBeenCalled();
    expect(h.editor.editing).toBe(true);
  });

  it('cancels boosted navigation before bubble listeners run', () => {
    const navSpy = vi.fn();
    document.body.addEventListener('htmx:beforeRequest', navSpy);
    vi.spyOn(window, 'confirm').mockReturnValue(false);
    h.editor.toggle();
    const textarea = h.container.querySelector('textarea')!;
    textarea.value = 'edited body';
    textarea.dispatchEvent(new Event('input', { bubbles: true }));
    const event = new CustomEvent('htmx:beforeRequest', {
      bubbles: true,
      cancelable: true,
      detail: { target: document.querySelector('article.markdown-body') },
    });

    textarea.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
    expect(navSpy).not.toHaveBeenCalled();
    expect(h.editor.editing).toBe(true);
  });

  it('cancels htmx history restoration when discard is declined', () => {
    const htmxHistory = vi.fn();
    window.onpopstate = htmxHistory;
    const pushSpy = vi.spyOn(history, 'pushState');
    vi.spyOn(window, 'confirm').mockReturnValue(false);
    h.editor.toggle();
    const textarea = h.container.querySelector('textarea')!;
    textarea.value = 'edited body';
    textarea.dispatchEvent(new Event('input', { bubbles: true }));

    window.dispatchEvent(new PopStateEvent('popstate', { state: {} }));

    expect(htmxHistory).not.toHaveBeenCalled();
    expect(pushSpy).toHaveBeenCalled();
    expect(h.editor.editing).toBe(true);
    window.onpopstate = null;
  });
});

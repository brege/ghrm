import {
  applyIndentKey,
  fitEditorHeight,
  repaintBlob,
  syncOverlayScroll,
} from './editor';
import { getWrapPref } from './prefs';

function rawSeed(pane: HTMLElement): string {
  return (
    (pane.querySelector('.ghrm-data') as HTMLTemplateElement | null)?.content
      ?.textContent ||
    pane.querySelector('.ghrm-blob-source code')?.textContent ||
    ''
  );
}

function editUrl(container: HTMLElement): string {
  return (container.dataset.ghrmRawUrl ?? '').replace(
    '/_ghrm/raw/',
    '/_ghrm/edit/',
  );
}

function parseVersion(value: string | null): string | null {
  if (!value?.startsWith('"') || !value.endsWith('"')) return null;
  const version = value.slice(1, -1);
  return version && !version.includes('"') ? version : null;
}

function versionTag(version: string): string {
  return `"${version}"`;
}

export async function readEditVersion(container: HTMLElement): Promise<string> {
  const response = await fetch(editUrl(container), {
    method: 'HEAD',
    cache: 'no-store',
  });
  const version = parseVersion(response.headers.get('ETag'));
  if (!response.ok || !version) {
    throw new Error(`edit version failed: ${response.status}`);
  }
  return version;
}

// Inline editor for the file view's raw/code pane. It overlays a transparent
// textarea on the server-rendered `.ghrm-blob`, driving the shared editor
// mechanics, and persists through PUT /_ghrm/edit/{path}. The textarea is
// created only when editing starts, so read-only file views ship no extra
// markup.
export class FileEditor {
  private textarea: HTMLTextAreaElement | null = null;
  private baseline = '';
  private saved = false;
  private controller: AbortController | null = null;
  private entryUrl = '';
  private entryState: unknown = null;

  constructor(
    private container: HTMLElement,
    private editUrl: string,
    private editBtn: HTMLButtonElement,
    private saveBtn: HTMLButtonElement,
    private rawToggle: HTMLButtonElement | null,
  ) {}

  private isSource(): boolean {
    return this.container.dataset.ghrmViewKind === 'source';
  }

  private pane(): HTMLElement | null {
    return this.container.querySelector('[data-ghrm-raw-pane]');
  }

  private blob(): HTMLElement | null {
    return this.container.querySelector('[data-ghrm-raw-pane] .ghrm-blob');
  }

  get editing(): boolean {
    return this.textarea !== null;
  }

  toggle(): void {
    if (this.editing) {
      this.exit();
    } else {
      this.enter();
    }
  }

  enter(): void {
    const pane = this.pane();
    const blob = this.blob();
    if (!pane || !blob || this.textarea) return;

    const textarea = document.createElement('textarea');
    textarea.className = 'ghrm-editor-input';
    textarea.value = rawSeed(pane);
    textarea.setAttribute('wrap', getWrapPref() ? 'soft' : 'off');
    textarea.autocomplete = 'off';
    textarea.spellcheck = false;
    textarea.setAttribute('aria-label', 'Edit file');
    pane.classList.add('ghrm-editor');
    pane.appendChild(textarea);
    this.textarea = textarea;
    this.baseline = textarea.value;
    this.saved = false;
    this.entryUrl = location.href;
    this.entryState = history.state;

    this.controller = new AbortController();
    const { signal } = this.controller;
    textarea.addEventListener('input', () => this.onChange(), { signal });
    textarea.addEventListener(
      'keydown',
      (event) => {
        if (applyIndentKey(event, textarea)) {
          this.onChange();
        }
      },
      { signal },
    );
    textarea.addEventListener(
      'scroll',
      () => syncOverlayScroll(textarea, blob),
      { signal },
    );
    window.addEventListener('resize', () => this.resizeSoon(), { signal });
    window.addEventListener('popstate', (event) => this.onHistory(event), {
      capture: true,
      signal,
    });
    window.addEventListener(
      'beforeunload',
      (event) => {
        if (this.dirty()) {
          event.preventDefault();
          event.returnValue = '';
        }
      },
      { signal },
    );
    document.body.addEventListener(
      'htmx:beforeRequest',
      (event) => this.onNavigate(event),
      { capture: true, signal },
    );

    this.container.dataset.ghrmEditing = '1';
    if (this.rawToggle) {
      this.rawToggle.disabled = true;
    }
    this.syncButtons();
    this.syncSave();
    this.resize();
    textarea.focus();
  }

  exit(): void {
    if (this.dirty() && !window.confirm('Discard unsaved changes?')) {
      return;
    }
    const reload = this.saved && !this.isSource();
    this.destroy();
    // A saved edit changes what a rendered preview should show; reload so the
    // preview reflects the file on disk. Source files have no preview to refresh.
    if (reload) {
      location.reload();
    }
  }

  // Tear down the editing session without prompting. Used when navigation has
  // already been confirmed, so listeners never outlive the swapped-out view.
  private destroy(): void {
    const pane = this.pane();
    const textarea = this.textarea;
    if (textarea) {
      // Restore the read-only blob to the on-disk content, dropping unsaved
      // edits that were mirrored into the overlay.
      if (pane && textarea.value !== this.baseline) {
        textarea.value = this.baseline;
        repaintBlob(pane, textarea);
      }
      textarea.remove();
      this.textarea = null;
    }
    this.controller?.abort();
    this.controller = null;
    pane?.classList.remove('ghrm-editor');
    delete this.container.dataset.ghrmEditing;
    delete this.container.dataset.ghrmEditDirty;
    delete this.container.dataset.ghrmEditConflict;
    delete this.container.dataset.ghrmEditConflictVersion;
    if (this.rawToggle && !this.isSource()) {
      this.rawToggle.disabled = false;
    }
    this.syncButtons();
  }

  // Boosted navigation swaps the file article; guard unsaved edits and tear the
  // session down so its global listeners never outlive the removed view.
  private onNavigate(event: Event): void {
    const detail = (event as CustomEvent).detail as
      | { target?: Element | null }
      | undefined;
    if (!detail?.target?.matches?.('article.markdown-body')) {
      return;
    }
    if (this.dirty() && !window.confirm('Discard unsaved changes?')) {
      event.preventDefault();
      event.stopImmediatePropagation();
      return;
    }
    this.destroy();
  }

  private onHistory(event: PopStateEvent): void {
    if (this.dirty() && !window.confirm('Discard unsaved changes?')) {
      event.stopImmediatePropagation();
      history.pushState(this.entryState, '', this.entryUrl);
      return;
    }
    this.destroy();
  }

  async save(): Promise<void> {
    const textarea = this.textarea;
    if (!textarea || !this.dirty()) return;
    const conflict = this.container.dataset.ghrmEditConflict === '1';
    if (
      conflict &&
      !window.confirm(
        'This file changed on disk since you started editing. Overwrite it?',
      )
    ) {
      return;
    }

    const text = textarea.value;
    const body =
      this.container.dataset.ghrmEol === 'crlf'
        ? text.replace(/\n/g, '\r\n')
        : text;
    this.saveBtn.disabled = true;
    this.saveBtn.setAttribute('aria-label', 'Saving');
    this.saveBtn.title = 'Saving';
    try {
      let expected = this.container.dataset.ghrmEditVersion;
      if (!expected) throw new Error('missing edit version');
      if (conflict) {
        expected =
          this.container.dataset.ghrmEditConflictVersion ??
          (await readEditVersion(this.container));
      }
      const response = await fetch(this.editUrl, {
        method: 'PUT',
        headers: {
          Accept: 'application/json',
          'Content-Type': 'text/plain; charset=utf-8',
          'If-Match': versionTag(expected),
        },
        body,
      });
      if (response.status === 412) {
        const current =
          parseVersion(response.headers.get('ETag')) ??
          (await readEditVersion(this.container));
        this.container.dataset.ghrmEditConflict = '1';
        this.container.dataset.ghrmEditConflictVersion = current;
        this.syncSave();
        return;
      }
      if (!response.ok) {
        throw new Error(`edit save failed: ${response.status}`);
      }
      const version = parseVersion(response.headers.get('ETag'));
      if (!version) throw new Error('edit save response omitted ETag');
      this.baseline = text;
      this.saved = true;
      this.container.dataset.ghrmEditVersion = version;
      delete this.container.dataset.ghrmEditConflict;
      delete this.container.dataset.ghrmEditConflictVersion;
      this.syncSave();
    } catch {
      this.saveBtn.setAttribute('aria-label', 'Save failed');
      this.saveBtn.title = 'Save failed';
      this.saveBtn.disabled = false;
    }
  }

  // Reflect the current wrap preference on the overlay and re-measure.
  onWrap(): void {
    if (!this.textarea) return;
    this.textarea.setAttribute('wrap', getWrapPref() ? 'soft' : 'off');
    this.resizeSoon();
  }

  private onChange(): void {
    const pane = this.pane();
    if (pane && this.textarea) {
      repaintBlob(pane, this.textarea);
    }
    this.syncSave();
    this.resizeSoon();
  }

  private dirty(): boolean {
    return !!this.textarea && this.textarea.value !== this.baseline;
  }

  private syncButtons(): void {
    this.editBtn.classList.toggle('is-active', this.editing);
    this.editBtn.setAttribute('aria-pressed', this.editing ? 'true' : 'false');
    const label = this.editing ? 'Stop editing' : 'Edit file';
    this.editBtn.setAttribute('aria-label', label);
    this.editBtn.title = label;
    this.editBtn.parentElement?.classList.toggle('is-editing', this.editing);
    this.saveBtn.hidden = !this.editing;
  }

  private syncSave(): void {
    const dirty = this.dirty();
    this.saveBtn.disabled = !dirty;
    const label =
      dirty && this.container.dataset.ghrmEditConflict === '1'
        ? 'Save and overwrite changed file'
        : dirty
          ? 'Save file'
          : 'No changes to save';
    this.saveBtn.setAttribute('aria-label', label);
    this.saveBtn.title = label;
    if (dirty) {
      this.container.dataset.ghrmEditDirty = '1';
    } else {
      delete this.container.dataset.ghrmEditDirty;
    }
  }

  private resize(): void {
    const pane = this.pane();
    const blob = this.blob();
    if (pane && this.textarea && blob) {
      fitEditorHeight(pane, this.textarea, blob);
    }
  }

  private resizeSoon(): void {
    requestAnimationFrame(() => this.resize());
  }
}

import { renderBlobs } from './adapters/code';
import { indentEdit } from './indent';

// Shared code-editor mechanics: a transparent textarea overlaid on a
// syntax-highlighted `.ghrm-blob`. Both the gist editor island and the file
// view's inline editor drive these so highlighting, sizing, scroll sync, and
// tab indentation stay identical across the two features.

// Size the transparent textarea to cover the highlighted blob it overlays.
export function fitEditorHeight(
  editor: HTMLElement,
  textarea: HTMLTextAreaElement,
  blob: HTMLElement,
): void {
  textarea.style.height = 'auto';
  const height = Math.max(
    textarea.scrollHeight,
    blob.offsetHeight,
    editor.clientHeight,
  );
  textarea.style.height = `${height}px`;
  blob.scrollLeft = textarea.scrollLeft;
}

// Copy the textarea text into the blob source and re-highlight, so the code
// under the transparent textarea always reflects the current input.
export function repaintBlob(
  root: ParentNode,
  textarea: HTMLTextAreaElement,
): void {
  const source = root.querySelector<HTMLElement>('.ghrm-blob-source code');
  const data = root.querySelector<HTMLTemplateElement>('template.ghrm-data');
  if (!source) return;

  const text = textarea.value;
  if (source.textContent !== text) {
    source.textContent = text;
    delete source.dataset.ghrmHighlighted;
  }
  if (data?.content) {
    data.content.textContent = text;
  }
  renderBlobs();
}

export function syncOverlayScroll(
  textarea: HTMLTextAreaElement,
  blob: HTMLElement,
): void {
  blob.scrollLeft = textarea.scrollLeft;
}

// Apply Tab/Shift-Tab indentation to the textarea selection. Returns true when
// the key was an indentation edit so the caller can repaint the blob.
export function applyIndentKey(
  event: KeyboardEvent,
  textarea: HTMLTextAreaElement,
): boolean {
  if (
    event.key !== 'Tab' ||
    event.altKey ||
    event.ctrlKey ||
    event.metaKey ||
    event.isComposing
  ) {
    return false;
  }

  event.preventDefault();
  const edit = indentEdit(
    textarea.value,
    textarea.selectionStart,
    textarea.selectionEnd,
    event.shiftKey,
  );
  textarea.setRangeText(edit.text, edit.start, edit.end, 'preserve');
  textarea.setSelectionRange(edit.selectionStart, edit.selectionEnd);
  return true;
}

export interface EditorSessionOptions {
  // Node holding `.ghrm-blob-source code` and the `template.ghrm-data` copy.
  root: ParentNode;
  textarea: HTMLTextAreaElement;
  blob: HTMLElement;
  // Element whose height bounds the overlay (the bordered editor container).
  sizeHost: HTMLElement;
  // Invoked after each edit so the caller can sync its own save controls.
  onChange: () => void;
}

// The shared editing surface: a transparent textarea overlaid on a highlighted
// `.ghrm-blob`, kept in sync (re-highlight, height, horizontal scroll, Tab
// indentation) with dirty tracking against a saved baseline. The gist editor
// and the file editor both compose this; resource-specific transport, save
// controls, and lifecycle stay with each caller. Listeners are owned by an
// AbortController exposed as `signal`, so a caller can bind its own listeners to
// the same lifetime and `destroy()` tears everything down at once.
export class EditorSession {
  readonly signal: AbortSignal;
  private controller = new AbortController();
  private baseline: string;

  constructor(private opts: EditorSessionOptions) {
    this.baseline = opts.textarea.value;
    this.signal = this.controller.signal;
    const { signal } = this.controller;
    const { textarea, blob } = opts;
    textarea.addEventListener('input', () => this.notifyChange(), { signal });
    textarea.addEventListener(
      'keydown',
      (event) => {
        if (applyIndentKey(event, textarea)) this.notifyChange();
      },
      { signal },
    );
    textarea.addEventListener(
      'scroll',
      () => syncOverlayScroll(textarea, blob),
      { signal },
    );
    window.addEventListener('resize', () => this.resizeSoon(), { signal });
  }

  // Re-highlight from the textarea and measure; call once after mounting.
  refresh(): void {
    this.repaint();
    this.resizeSoon();
  }

  private notifyChange(): void {
    this.repaint();
    this.opts.onChange();
    this.resizeSoon();
  }

  private repaint(): void {
    repaintBlob(this.opts.root, this.opts.textarea);
  }

  resize(): void {
    fitEditorHeight(this.opts.sizeHost, this.opts.textarea, this.opts.blob);
  }

  resizeSoon(): void {
    requestAnimationFrame(() => this.resize());
  }

  applyWrap(wrap: boolean): void {
    this.opts.textarea.setAttribute('wrap', wrap ? 'soft' : 'off');
    this.resizeSoon();
  }

  get value(): string {
    return this.opts.textarea.value;
  }

  get dirty(): boolean {
    return this.opts.textarea.value !== this.baseline;
  }

  markSaved(value: string = this.opts.textarea.value): void {
    this.baseline = value;
  }

  // Drop unsaved edits, restoring the blob to the saved baseline.
  restore(): void {
    if (this.opts.textarea.value !== this.baseline) {
      this.opts.textarea.value = this.baseline;
      this.repaint();
    }
  }

  destroy(): void {
    this.controller.abort();
  }
}

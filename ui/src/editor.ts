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

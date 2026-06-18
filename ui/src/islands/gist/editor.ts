import { LitElement } from 'lit';
import { renderBlobs } from '../../adapters/code';
import { showCopied, writeClipboard } from '../../adapters/copy';
import { populateDates } from '../../explorer';
import { indentEdit } from '../../indent';
import { applyWrapState, getWrapPref, setWrapPref } from '../../prefs';

interface GistNameInput extends HTMLInputElement {
  dataset: DOMStringMap & {
    ghrmGistSaved?: string;
  };
}

interface GistTextarea extends HTMLTextAreaElement {
  dataset: DOMStringMap & {
    ghrmGistSaved?: string;
  };
}

interface GistCopyButton extends HTMLButtonElement {
  _ghrmCopyReset?: number | null;
}

const gistPath = '/_ghrm/gist';
const nameMax = 80;

function deleteUrl(id: string): string {
  return `/_ghrm/gist/p/${encodeURIComponent(id)}`;
}

function pad(value: number, width: number): string {
  return String(value).padStart(width, '0');
}

function defaultGistName(): string {
  const now = new Date();
  return `${now.getUTCFullYear()}${pad(now.getUTCMonth() + 1, 2)}${pad(now.getUTCDate(), 2)}T${pad(now.getUTCHours(), 2)}${pad(now.getUTCMinutes(), 2)}${pad(now.getUTCSeconds(), 2)}.${pad(now.getUTCMilliseconds(), 3)}000000Z`;
}

function normalizeName(value: string): string {
  const name = value.trim();
  return name.endsWith('.txt') ? name.slice(0, -4) : name;
}

function validName(name: string): boolean {
  return (
    name.length <= nameMax &&
    (name === '' ||
      (name !== '.' &&
        name !== '..' &&
        !name.startsWith('.') &&
        !name.endsWith('.') &&
        /^[A-Za-z0-9._-]+$/.test(name)))
  );
}

export class GhrmGistEditor extends LitElement {
  private boundLiveHandler: (() => void) | null = null;
  private boundResizeHandler: (() => void) | null = null;
  private connectedOnce = false;
  private pendingRefresh = false;

  protected createRenderRoot(): HTMLElement {
    return this;
  }

  connectedCallback(): void {
    super.connectedCallback();
    this.setupEditor();
    if (!this.connectedOnce) {
      this.connectedOnce = true;
      this.addGlobalListeners();
    }
  }

  disconnectedCallback(): void {
    super.disconnectedCallback();
    this.removeGlobalListeners();
    this.connectedOnce = false;
  }

  private getArticle(): HTMLElement | null {
    return this.closest('article[data-ghrm-gist]');
  }

  private getGistId(): string | undefined {
    return this.getArticle()?.dataset.ghrmGistId;
  }

  private getGistPage(): string {
    return this.getArticle()?.dataset.ghrmGistPage || gistPath;
  }

  private getTextarea(): GistTextarea | null {
    return this.querySelector<GistTextarea>('[data-ghrm-gist-form] textarea');
  }

  private getNameInput(): GistNameInput | null {
    return this.querySelector<GistNameInput>('[data-ghrm-gist-name]');
  }

  private currentText(): string {
    return this.getTextarea()?.value || '';
  }

  private hasUnsavedChanges(): boolean {
    const input = this.getTextarea();
    if (!input) return false;
    const name = this.getNameInput();
    const normalized = name ? normalizeName(name.value) : '';
    return (
      input.value !== input.dataset.ghrmGistSaved ||
      !!(name && normalized !== name.dataset.ghrmGistSaved)
    );
  }

  private setStatus(message: string): void {
    const status = this.querySelector<HTMLElement>('[data-ghrm-gist-status]');
    if (status) {
      status.textContent = message;
    }
  }

  private syncSaveAction(saving = false): void {
    const input = this.getTextarea();
    const name = this.getNameInput();
    const control = this.querySelector<HTMLElement>(
      '[data-ghrm-gist-save-control]',
    );
    const button = this.querySelector<HTMLButtonElement>(
      '[data-ghrm-gist-save]',
    );
    if (!input || !button) return;

    const normalized = name ? normalizeName(name.value) : '';
    const valid = !name || validName(normalized);
    const changed =
      input.value !== input.dataset.ghrmGistSaved ||
      !!(name && normalized !== name.dataset.ghrmGistSaved);
    button.disabled = saving || !valid || !changed;
    const label = saving
      ? 'Saving'
      : !valid
        ? 'Use letters, numbers, dots, dashes, or underscores'
        : changed
          ? 'Save paste'
          : 'No changes to save';
    button.setAttribute('aria-label', label);
    button.title = label;
    name?.setAttribute('aria-invalid', valid ? 'false' : 'true');
    if (control) {
      control.title = label;
    }
  }

  private syncEditor(): void {
    const editor = this.querySelector<HTMLElement>('[data-ghrm-gist-editor]');
    const input = this.getTextarea();
    const blob = this.querySelector<HTMLElement>('.ghrm-blob');
    if (!editor || !input || !blob) return;

    input.style.height = 'auto';
    const height = Math.max(
      input.scrollHeight,
      blob.offsetHeight,
      editor.clientHeight,
    );
    input.style.height = `${height}px`;
    blob.scrollLeft = input.scrollLeft;
  }

  private syncEditorSoon(): void {
    requestAnimationFrame(() => {
      this.syncEditor();
    });
  }

  private syncBlob(): void {
    const input = this.getTextarea();
    const source = this.querySelector<HTMLElement>('.ghrm-blob-source code');
    const data = this.querySelector<HTMLTemplateElement>('template.ghrm-data');
    if (!input || !source) return;

    const text = input.value;
    if (source.textContent !== text) {
      source.textContent = text;
      delete source.dataset.ghrmHighlighted;
    }
    if (data?.content) {
      data.content.textContent = text;
    }

    renderBlobs();
    this.syncSaveAction();
    this.syncEditorSoon();
  }

  private syncBlobScroll(): void {
    const input = this.getTextarea();
    const blob = this.querySelector<HTMLElement>('.ghrm-blob');
    if (!input || !blob) return;
    blob.scrollLeft = input.scrollLeft;
  }

  private handleIndentKey(event: KeyboardEvent): void {
    if (
      event.key !== 'Tab' ||
      event.altKey ||
      event.ctrlKey ||
      event.metaKey ||
      event.isComposing
    ) {
      return;
    }

    event.preventDefault();
    const input = event.currentTarget as GistTextarea;
    const edit = indentEdit(
      input.value,
      input.selectionStart,
      input.selectionEnd,
      event.shiftKey,
    );
    input.setRangeText(edit.text, edit.start, edit.end, 'preserve');
    input.setSelectionRange(edit.selectionStart, edit.selectionEnd);
    this.syncBlob();
  }

  private syncWrapToggle(): void {
    const toggle = this.querySelector<HTMLElement>('[data-ghrm-gist-wrap]');
    const input = this.getTextarea();
    if (!toggle || !input) return;

    const wrap = getWrapPref();
    toggle.classList.toggle('is-active', wrap);
    toggle.setAttribute('aria-pressed', wrap ? 'true' : 'false');
    const label = wrap ? 'Disable line wrap' : 'Wrap lines';
    toggle.setAttribute('aria-label', label);
    toggle.title = label;
    input.setAttribute('wrap', wrap ? 'soft' : 'off');
    applyWrapState(wrap);
    this.syncEditorSoon();
  }

  private replaceGistUrl(): void {
    if (window.location.pathname !== gistPath) {
      window.history.replaceState(window.history.state, '', gistPath);
    }
  }

  private async save(): Promise<void> {
    const input = this.getTextarea();
    if (!input) return;
    const name = this.getNameInput();
    const normalized = name ? normalizeName(name.value) : '';
    if (name && !validName(normalized)) {
      this.syncSaveAction();
      return;
    }
    if (
      input.value === input.dataset.ghrmGistSaved &&
      (!name || normalized === name.dataset.ghrmGistSaved)
    ) {
      this.syncSaveAction();
      return;
    }
    this.syncSaveAction(true);
    this.setStatus('Saving');
    const headers: Record<string, string> = {
      Accept: 'application/json',
      'Content-Type': 'text/plain; charset=utf-8',
    };
    if (normalized) {
      headers['X-Ghrm-Gist-Name'] = normalized;
    }
    const gistId = this.getGistId();
    if (gistId) {
      headers['X-Ghrm-Gist-Id'] = gistId;
    }
    try {
      const response = await fetch(gistPath, {
        method: 'POST',
        headers,
        body: input.value,
      });
      if (!response.ok) {
        throw new Error(`gist save failed: ${response.status}`);
      }
      const refreshed = await this.refresh(gistPath, 'Saved');
      if (refreshed) {
        this.replaceGistUrl();
      } else {
        this.syncSaveAction();
      }
    } catch {
      this.setStatus('Save failed');
      this.syncSaveAction();
    }
  }

  private async deletePaste(): Promise<void> {
    const gistId = this.getGistId();
    if (!gistId) return;

    const confirmed = window.confirm(
      'Delete this paste? This cannot be undone.',
    );
    if (!confirmed) return;

    this.setStatus('Deleting');
    try {
      const response = await fetch(deleteUrl(gistId), { method: 'DELETE' });
      if (!response.ok) {
        throw new Error(`delete failed: ${response.status}`);
      }
      this.setStatus('Deleted');
      await this.refresh(gistPath, 'Deleted');
      this.replaceGistUrl();
    } catch {
      this.setStatus('Delete failed');
    }
  }

  private async newPaste(): Promise<void> {
    if (this.hasUnsavedChanges()) {
      const confirmed = window.confirm(
        'Discard unsaved changes and create a new paste?',
      );
      if (!confirmed) return;
    }
    await this.refresh(`${gistPath}?new=true`);
    window.history.replaceState(window.history.state, '', gistPath);
  }

  async refresh(path = this.getGistPage(), status?: string): Promise<boolean> {
    const article = this.getArticle();
    if (!article) return false;

    const response = await fetch(path, {
      headers: {
        Accept: 'text/html',
        'HX-Request': 'true',
      },
    });
    if (!response.ok) {
      this.setStatus('Refresh failed');
      return false;
    }

    const html = await response.text();
    const doc = new DOMParser().parseFromString(html, 'text/html');
    const next = doc.querySelector('article[data-ghrm-gist]');
    if (!next) {
      this.setStatus('Refresh failed');
      return false;
    }

    if (status) {
      const nextStatus = next.querySelector<HTMLElement>(
        '[data-ghrm-gist-status]',
      );
      if (nextStatus) {
        nextStatus.textContent = status;
      }
    }
    article.replaceWith(next);
    this.pendingRefresh = false;
    populateDates();
    document.dispatchEvent(new CustomEvent('ghrm:contentready'));
    return true;
  }

  private refreshPending(): void {
    if (!this.pendingRefresh || this.hasUnsavedChanges()) return;
    this.refresh();
  }

  private requestRefresh(): void {
    if (this.hasUnsavedChanges()) {
      this.pendingRefresh = true;
      return;
    }
    this.refresh();
  }

  private setupEditor(): void {
    const article = this.getArticle();
    if (!article || article.dataset.ghrmGistReady === '1') return;
    article.dataset.ghrmGistReady = '1';

    const form = this.querySelector<HTMLFormElement>('[data-ghrm-gist-form]');
    form?.addEventListener('submit', (event) => {
      event.preventDefault();
      this.save();
    });

    const saveButton = this.querySelector<HTMLButtonElement>(
      '[data-ghrm-gist-save]',
    );
    saveButton?.addEventListener('click', () => {
      this.save();
    });

    const input = this.getTextarea();
    if (input) {
      input.dataset.ghrmGistSaved = input.value;
    }
    const name = this.getNameInput();
    if (name) {
      if (!name.value) {
        name.value = defaultGistName();
      }
      name.dataset.ghrmGistSaved =
        this.getGistId() || normalizeName(name.value);
      name.addEventListener('input', () => {
        this.syncSaveAction();
        this.refreshPending();
      });
    }
    input?.addEventListener('input', () => {
      this.syncBlob();
      this.refreshPending();
    });
    input?.addEventListener('keydown', (event) => {
      this.handleIndentKey(event);
    });
    input?.addEventListener('scroll', () => {
      this.syncBlobScroll();
    });

    const copy = this.querySelector<GistCopyButton>('[data-ghrm-gist-copy]');
    copy?.addEventListener('click', async () => {
      await writeClipboard(this.currentText());
      showCopied(copy);
    });

    const wrap = this.querySelector<HTMLElement>('[data-ghrm-gist-wrap]');
    wrap?.addEventListener('click', () => {
      setWrapPref(!getWrapPref());
      this.syncWrapToggle();
    });

    const newButton = this.querySelector<HTMLButtonElement>(
      '[data-ghrm-gist-new]',
    );
    newButton?.addEventListener('click', () => {
      this.newPaste();
    });

    const deleteButton = this.querySelector<HTMLButtonElement>(
      '[data-ghrm-gist-delete]',
    );
    deleteButton?.addEventListener('click', () => {
      this.deletePaste();
    });

    this.syncWrapToggle();
    this.syncSaveAction();
    renderBlobs();
    this.syncEditorSoon();
  }

  private addGlobalListeners(): void {
    this.boundLiveHandler = () => {
      this.requestRefresh();
    };
    this.boundResizeHandler = () => {
      this.syncEditorSoon();
    };
    document.addEventListener('ghrm:live:gist', this.boundLiveHandler);
    window.addEventListener('resize', this.boundResizeHandler);
  }

  private removeGlobalListeners(): void {
    if (this.boundLiveHandler) {
      document.removeEventListener('ghrm:live:gist', this.boundLiveHandler);
      this.boundLiveHandler = null;
    }
    if (this.boundResizeHandler) {
      window.removeEventListener('resize', this.boundResizeHandler);
      this.boundResizeHandler = null;
    }
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'ghrm-gist-editor': GhrmGistEditor;
  }
}

customElements.define('ghrm-gist-editor', GhrmGistEditor);

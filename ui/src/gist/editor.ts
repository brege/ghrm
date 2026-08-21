import { LitElement } from 'lit';
import { showCopied, writeClipboard } from '../adapters/copy';
import { EditorSession } from '../editor/editor';
import { swapArticle } from '../shell/nav';
import { applyWrapState, getWrapPref, setWrapPref } from '../shell/prefs';
import { normalizeName, validName } from './name';

interface GistNameInput extends HTMLInputElement {
  dataset: DOMStringMap & {
    ghrmGistSaved?: string;
  };
}

interface GistCopyButton extends HTMLButtonElement {
  _ghrmCopyReset?: number | null;
}

const gistPath = '/_ghrm/gist';

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

export class GhrmGistEditor extends LitElement {
  private session: EditorSession | null = null;
  private boundLiveHandler: (() => void) | null = null;
  private connectedOnce = false;
  private pendingRefresh = false;

  protected createRenderRoot(): HTMLElement {
    return this;
  }

  connectedCallback(): void {
    super.connectedCallback();
    this.setupControls();
    this.ensureSession();
    if (!this.connectedOnce) {
      this.connectedOnce = true;
      this.addGlobalListeners();
    }
  }

  disconnectedCallback(): void {
    super.disconnectedCallback();
    this.session?.destroy();
    this.session = null;
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

  private getTextarea(): HTMLTextAreaElement | null {
    return this.querySelector<HTMLTextAreaElement>(
      '[data-ghrm-gist-form] textarea',
    );
  }

  private getNameInput(): GistNameInput | null {
    return this.querySelector<GistNameInput>('[data-ghrm-gist-name]');
  }

  private getSaveButtons(): HTMLButtonElement[] {
    return [
      ...this.querySelectorAll<HTMLButtonElement>('[data-ghrm-gist-save]'),
    ];
  }

  private currentText(): string {
    return this.session?.value ?? this.getTextarea()?.value ?? '';
  }

  private nameChanged(): boolean {
    const name = this.getNameInput();
    return !!(name && normalizeName(name.value) !== name.dataset.ghrmGistSaved);
  }

  private hasUnsavedChanges(): boolean {
    return (this.session?.dirty ?? false) || this.nameChanged();
  }

  private setStatus(message: string): void {
    const status = this.querySelector<HTMLElement>('[data-ghrm-gist-status]');
    if (status) {
      status.textContent = message;
    }
  }

  private syncSaveAction(saving = false): void {
    const buttons = this.getSaveButtons();
    if (buttons.length === 0) return;
    const name = this.getNameInput();
    const controls = this.querySelectorAll<HTMLElement>(
      '[data-ghrm-gist-save-control]',
    );

    const normalized = name ? normalizeName(name.value) : '';
    const valid = !name || validName(normalized);
    const changed = (this.session?.dirty ?? false) || this.nameChanged();
    const label = saving
      ? 'Saving'
      : !valid
        ? 'Use letters, numbers, dots, dashes, or underscores'
        : changed
          ? 'Save paste'
          : 'No changes to save';
    for (const button of buttons) {
      button.disabled = saving || !valid || !changed;
      button.setAttribute('aria-label', label);
      button.title = label;
    }
    name?.setAttribute('aria-invalid', valid ? 'false' : 'true');
    for (const control of controls) {
      control.title = label;
    }
  }

  private syncWrapToggle(): void {
    const toggle = this.querySelector<HTMLElement>('[data-ghrm-gist-wrap]');
    if (!toggle) return;

    const wrap = getWrapPref();
    toggle.classList.toggle('is-active', wrap);
    toggle.setAttribute('aria-pressed', wrap ? 'true' : 'false');
    const label = wrap ? 'Disable line wrap' : 'Wrap lines';
    toggle.setAttribute('aria-label', label);
    toggle.title = label;
    applyWrapState(wrap);
    this.session?.applyWrap(wrap);
  }

  private replaceGistUrl(): void {
    if (window.location.pathname !== gistPath) {
      window.history.replaceState(window.history.state, '', gistPath);
    }
  }

  private async save(): Promise<void> {
    const name = this.getNameInput();
    const normalized = name ? normalizeName(name.value) : '';
    if (name && !validName(normalized)) {
      this.syncSaveAction();
      return;
    }
    if (!this.hasUnsavedChanges()) {
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
        body: this.currentText(),
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

    const next = await swapArticle(
      article,
      path,
      'article[data-ghrm-gist]',
      (fragment) => {
        if (!status) return;
        const nextStatus = fragment.querySelector<HTMLElement>(
          '[data-ghrm-gist-status]',
        );
        if (nextStatus) {
          nextStatus.textContent = status;
        }
      },
    );
    if (!next) {
      this.setStatus('Refresh failed');
      return false;
    }
    this.pendingRefresh = false;
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

  // One-time control wiring for the paste article. Guarded so a reconnecting
  // element does not double-bind these listeners.
  private setupControls(): void {
    const article = this.getArticle();
    if (!article || article.dataset.ghrmGistReady === '1') return;
    article.dataset.ghrmGistReady = '1';

    const form = this.querySelector<HTMLFormElement>('[data-ghrm-gist-form]');
    form?.addEventListener('submit', (event) => {
      event.preventDefault();
      this.save();
    });

    for (const button of this.getSaveButtons()) {
      button.addEventListener('click', () => {
        this.save();
      });
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

    this.syncSaveAction();
  }

  // The editing session owns the textarea overlay listeners, which are released
  // on disconnect. Create a fresh session whenever the element connects without
  // one so reconnection restores live editing.
  private ensureSession(): void {
    if (this.session) return;
    const input = this.getTextarea();
    const blob = this.querySelector<HTMLElement>('.ghrm-blob');
    const host = this.querySelector<HTMLElement>('[data-ghrm-gist-editor]');
    if (!input || !blob || !host) return;

    this.session = new EditorSession({
      root: this,
      textarea: input,
      blob,
      sizeHost: host,
      onChange: () => {
        this.syncSaveAction();
        this.refreshPending();
      },
    });
    this.syncWrapToggle();
    this.syncSaveAction();
    this.session.refresh();
  }

  private addGlobalListeners(): void {
    this.boundLiveHandler = () => {
      this.requestRefresh();
    };
    document.addEventListener('ghrm:live:gist', this.boundLiveHandler);
  }

  private removeGlobalListeners(): void {
    if (this.boundLiveHandler) {
      document.removeEventListener('ghrm:live:gist', this.boundLiveHandler);
      this.boundLiveHandler = null;
    }
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'ghrm-gist-editor': GhrmGistEditor;
  }
}

customElements.define('ghrm-gist-editor', GhrmGistEditor);

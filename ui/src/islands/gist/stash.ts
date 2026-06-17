import { LitElement } from 'lit';
import { populateDates } from '../../explorer';

interface GistRow extends HTMLElement {
  dataset: DOMStringMap & {
    ghrmGistId: string;
  };
}

interface GistRowInput extends HTMLInputElement {
  dataset: DOMStringMap & {
    ghrmGistRowInput?: string;
    ghrmSaving?: string;
  };
}

interface RenameResponse {
  id: string;
  href: string;
  name: string;
}

const stashPath = '/_ghrm/gist/stash';
const nameMax = 80;

function deleteUrl(id: string): string {
  return `/_ghrm/gist/p/${encodeURIComponent(id)}`;
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

function rowRenameUrl(row: GistRow): string {
  return `/_ghrm/gist/rename/${encodeURIComponent(row.dataset.ghrmGistId)}`;
}

function renameResponse(value: unknown): RenameResponse {
  if (
    typeof value !== 'object' ||
    value === null ||
    typeof (value as RenameResponse).id !== 'string' ||
    typeof (value as RenameResponse).href !== 'string' ||
    typeof (value as RenameResponse).name !== 'string'
  ) {
    throw new Error('invalid gist rename response');
  }
  return value as RenameResponse;
}

export class GhrmGistStash extends LitElement {
  private boundLiveHandler: (() => void) | null = null;
  private connectedOnce = false;

  protected createRenderRoot(): HTMLElement {
    return this;
  }

  connectedCallback(): void {
    super.connectedCallback();
    this.setupRows();
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
    return this.closest('article[data-ghrm-gist-stash]');
  }

  private setupRows(): void {
    const article = this.getArticle();
    if (!article) return;
    for (const row of article.querySelectorAll<GistRow>(
      '[data-ghrm-gist-row]',
    )) {
      const renameButton = row.querySelector<HTMLElement>(
        '[data-ghrm-gist-rename-start]',
      );
      if (renameButton && !renameButton.dataset.ghrmBound) {
        renameButton.dataset.ghrmBound = '1';
        renameButton.addEventListener('click', () => {
          this.beginRowRename(row);
        });
      }
      const deleteButton = row.querySelector<HTMLElement>(
        '[data-ghrm-gist-delete-start]',
      );
      if (deleteButton && !deleteButton.dataset.ghrmBound) {
        deleteButton.dataset.ghrmBound = '1';
        deleteButton.addEventListener('click', () => {
          this.deleteRow(row);
        });
      }
    }
  }

  private async deleteRow(row: GistRow): Promise<void> {
    const id = row.dataset.ghrmGistId;
    if (!id) return;

    const link = row.querySelector<HTMLElement>('[data-ghrm-gist-row-link]');
    const name = link?.textContent || id;
    const confirmed = window.confirm(
      `Delete "${name}"? This cannot be undone.`,
    );
    if (!confirmed) return;

    const response = await fetch(deleteUrl(id), { method: 'DELETE' });
    if (!response.ok) return;

    await this.refresh();
  }

  private restoreRowRename(cell: Element, input: GistRowInput): void {
    const link = cell.querySelector<HTMLElement>('[data-ghrm-gist-row-link]');
    const button = cell.querySelector<HTMLElement>(
      '[data-ghrm-gist-rename-start]',
    );
    input.remove();
    if (link) {
      link.hidden = false;
    }
    if (button) {
      button.hidden = false;
    }
  }

  private async saveRowRename(
    row: GistRow,
    cell: Element,
    input: GistRowInput,
  ): Promise<void> {
    if (input.dataset.ghrmSaving === '1') return;
    const link = cell.querySelector<HTMLAnchorElement>(
      '[data-ghrm-gist-row-link]',
    );
    const next = normalizeName(input.value);
    const current = normalizeName(link?.textContent || '');
    if (!validName(next)) {
      input.setAttribute('aria-invalid', 'true');
      input.title = 'Use letters, numbers, dots, dashes, or underscores';
      input.focus();
      return;
    }
    if (next === current) {
      this.restoreRowRename(cell, input);
      return;
    }

    input.dataset.ghrmSaving = '1';
    const response = await fetch(rowRenameUrl(row), {
      method: 'POST',
      headers: {
        Accept: 'application/json',
        'Content-Type': 'text/plain; charset=utf-8',
      },
      body: next,
    });
    if (!response.ok) {
      input.dataset.ghrmSaving = '0';
      input.setAttribute('aria-invalid', 'true');
      input.title = 'Name already exists or is invalid';
      input.focus();
      return;
    }

    const renamed = renameResponse(await response.json());
    row.dataset.ghrmGistId = renamed.id;
    if (link) {
      link.href = renamed.href;
      link.textContent = renamed.name;
    }
    this.restoreRowRename(cell, input);
  }

  private beginRowRename(row: GistRow): void {
    const cell = row.querySelector('.ghrm-gist-name-cell');
    const link = cell?.querySelector<HTMLAnchorElement>(
      '[data-ghrm-gist-row-link]',
    );
    const button = cell?.querySelector<HTMLElement>(
      '[data-ghrm-gist-rename-start]',
    );
    if (!cell || !link || cell.querySelector('[data-ghrm-gist-row-input]'))
      return;

    const input = document.createElement('input') as GistRowInput;
    input.type = 'text';
    input.className = 'ghrm-gist-row-input';
    input.dataset.ghrmGistRowInput = '1';
    input.value = link.textContent || '';
    input.setAttribute('aria-label', 'Paste filename');
    input.autocomplete = 'off';
    input.spellcheck = false;

    link.hidden = true;
    if (button) {
      button.hidden = true;
    }
    cell.insertBefore(input, link);
    input.focus();
    input.select();

    input.addEventListener('keydown', (event) => {
      if (event.key === 'Enter') {
        event.preventDefault();
        this.saveRowRename(row, cell, input);
      } else if (event.key === 'Escape') {
        event.preventDefault();
        this.restoreRowRename(cell, input);
      }
    });
    input.addEventListener('blur', () => {
      if (input.isConnected) {
        this.saveRowRename(row, cell, input);
      }
    });
  }

  async refresh(): Promise<void> {
    const article = this.getArticle();
    if (!article) return;

    const response = await fetch(stashPath, {
      headers: {
        Accept: 'text/html',
        'HX-Request': 'true',
      },
    });
    if (!response.ok) return;

    const html = await response.text();
    const doc = new DOMParser().parseFromString(html, 'text/html');
    const next = doc.querySelector('article[data-ghrm-gist-stash]');
    if (!next) return;

    article.replaceWith(next);
    populateDates();
    document.dispatchEvent(new CustomEvent('ghrm:contentready'));
  }

  private addGlobalListeners(): void {
    this.boundLiveHandler = () => {
      this.refresh();
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
    'ghrm-gist-stash': GhrmGistStash;
  }
}

customElements.define('ghrm-gist-stash', GhrmGistStash);

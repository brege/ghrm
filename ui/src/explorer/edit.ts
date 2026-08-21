import { encodePath, qsel } from '../dom';
import { beginInlineRename, validFileName } from '../editor/rename';
import { EDIT_PENDING_KEY } from '../file/file';
import { ExpandField, expandParts } from '../shell/expand';
import { swapArticle } from '../shell/nav';

const invalidName = 'Use a file name without path separators';

function editHref(rel: string): string {
  return `/_ghrm/edit/${encodePath(rel)}`;
}

function fileHref(rel: string): string {
  return `/${encodePath(rel)}`;
}

interface ExplorerEditIo {
  navigate: (href: string) => void;
  refresh: (article: HTMLElement) => Promise<void>;
}

const defaultIo: ExplorerEditIo = {
  navigate: (href) => location.assign(href),
  refresh: async (article) => {
    const href = `${location.pathname}${location.search}`;
    try {
      if (await swapArticle(article, href, 'article[data-explorer]')) return;
    } catch {
      location.assign(href);
      return;
    }
    location.assign(href);
  },
};

// Explorer mutation controls, server-rendered only when the edit feature is
// active: a new-file control in the header and rename/delete hover controls
// on file rows, driving the shared inline-rename lifecycle and the
// /_ghrm/edit resource.
export function setupExplorerEdit(): void {
  bindExplorerEdit(defaultIo);
}

export function bindExplorerEdit(io: ExplorerEditIo): void {
  const article = qsel('article[data-explorer]');
  if (!article) return;
  bindNewFile(article, io);
  for (const seat of article.querySelectorAll<HTMLElement>(
    '[data-ghrm-row-path]',
  )) {
    bindRow(article, seat, io);
  }
}

// The header new-file control uses the same expanding field as the topbar path
// search: the toggle opens an input that grows left over the breadcrumbs, Enter
// creates the file, and a rejected name keeps the field open for a retry.
function bindNewFile(article: HTMLElement, io: ExplorerEditIo): void {
  const root = article.querySelector<HTMLElement>('[data-ghrm-new-file]');
  if (!root || root.dataset.ghrmBound) return;
  const parts = expandParts(root);
  if (!parts) return;
  root.dataset.ghrmBound = '1';

  const field: ExpandField = new ExpandField({
    root,
    input: parts.input,
    toggle: parts.toggle,
    onClose: () => {
      field.input.value = '';
    },
    onEnter: () => {
      void createFile(article, field, io);
    },
  });
}

async function createFile(
  article: HTMLElement,
  field: ExpandField,
  io: ExplorerEditIo,
): Promise<void> {
  if (field.input.dataset.ghrmSaving === '1') return;
  const name = field.value.trim();
  if (!validFileName(name)) {
    field.invalid(invalidName);
    return;
  }

  const dir = (article.dataset.currentPath ?? '').replace(/^\/+|\/+$/g, '');
  const rel = dir ? `${dir}/${name}` : name;
  field.input.dataset.ghrmSaving = '1';
  let response: Response;
  try {
    response = await fetch(editHref(rel), {
      method: 'PUT',
      headers: {
        Accept: 'application/json',
        'Content-Type': 'text/plain; charset=utf-8',
        'If-None-Match': '*',
      },
      body: '',
    });
  } catch {
    field.input.dataset.ghrmSaving = '0';
    field.invalid('Create failed');
    return;
  }
  if (!response.ok) {
    field.input.dataset.ghrmSaving = '0';
    field.invalid(
      response.status === 412 ? 'File already exists' : 'Create failed',
    );
    return;
  }
  // Open the new file directly in edit mode after navigation.
  sessionStorage.setItem(EDIT_PENDING_KEY, rel);
  io.navigate(`${fileHref(rel)}${location.search}`);
}

function bindRow(
  article: HTMLElement,
  seat: HTMLElement,
  io: ExplorerEditIo,
): void {
  if (seat.dataset.ghrmBound) return;
  seat.dataset.ghrmBound = '1';
  const cell = seat.closest('td');
  const link = cell?.querySelector('a');
  const renameButton = seat.querySelector<HTMLElement>(
    '[data-ghrm-row-rename]',
  );
  const deleteButton = seat.querySelector<HTMLElement>(
    '[data-ghrm-row-delete]',
  );
  if (!cell || !link) return;

  renameButton?.addEventListener('click', () => {
    const rel = seat.dataset.ghrmRowPath ?? '';
    beginInlineRename({
      anchor: link,
      value: link.textContent ?? '',
      label: 'File name',
      hide: [link, seat],
      invalidTitle: invalidName,
      errorTitle: 'Rename failed',
      validate: validFileName,
      submit: async (name) => {
        const response = await fetch(editHref(rel), {
          method: 'PATCH',
          headers: {
            Accept: 'application/json',
            'Content-Type': 'text/plain; charset=utf-8',
          },
          body: name,
        });
        if (!response.ok) {
          return {
            ok: false,
            message:
              response.status === 409 ? 'Name already exists' : 'Rename failed',
          };
        }
        await io.refresh(article);
        return { ok: true };
      },
    });
  });

  deleteButton?.addEventListener('click', async () => {
    const rel = seat.dataset.ghrmRowPath ?? '';
    const name = link.textContent || rel;
    if (!window.confirm(`Delete "${name}"? This cannot be undone.`)) return;
    const response = await fetch(editHref(rel), { method: 'DELETE' });
    if (!response.ok) return;
    await io.refresh(article);
  });
}

import { encodePath, qsel } from './dom';
import { readEditVersion } from './file/file-edit';
import { refreshActiveSearch } from './search/search';
import { setConnected } from './shell/status';

export interface LiveEvent {
  name: string;
  path: string | null;
}

export interface ContentPath {
  kind: 'dir' | 'file';
  path: string;
}

export function setupLiveReload(): void {
  const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  const url = `${proto}//${location.host}/_ghrm/ws`;
  let connectedOnce = false;
  function connect() {
    const ws = new WebSocket(url);
    ws.onopen = () => {
      setConnected(true);
      if (connectedOnce) {
        void reloadAfterReconnect();
        return;
      }
      connectedOnce = true;
    };
    ws.onmessage = (ev) => {
      void handleLiveEvent(ev.data);
    };
    ws.onerror = () => {
      setConnected(false);
    };
    ws.onclose = () => {
      setConnected(false);
      setTimeout(connect, 1000);
    };
  }
  connect();
}

export function parseLiveMessage(message: string): LiveEvent {
  const reloadPrefix = 'reload:';
  if (message.startsWith(reloadPrefix)) {
    return {
      name: 'reload',
      path: decodeURIComponent(message.slice(reloadPrefix.length)),
    };
  }
  return { name: message, path: null };
}

export function cleanRelPath(path: string): string {
  return stripTrailingSlashes(stripLeadingSlashes(path));
}

export function shouldReloadForChange(
  current: ContentPath | null,
  path: string | null,
): boolean {
  if (!current) return path === null;
  if (path === null) return true;

  const changed = cleanRelPath(path);
  if (!changed) return false;
  if (current.kind === 'file') return changed === current.path;
  return parentPath(changed) === current.path;
}

export function shouldNavigateToParent(
  current: ContentPath | null,
  path: string | null,
): boolean {
  if (!current || path === null) return false;
  const changed = cleanRelPath(path);
  const active = cleanRelPath(current.path);
  return (
    current.kind === 'dir' &&
    changed !== '' &&
    (active === changed || active.startsWith(`${changed}/`))
  );
}

// The shell of an in-progress file edit, or null when nothing is being edited.
function editingShell(): HTMLElement | null {
  return document.querySelector<HTMLElement>(
    '.ghrm-page-shell[data-ghrm-view-kind][data-ghrm-editing]',
  );
}

function currentContentPath(): ContentPath | null {
  const explorer = qsel('article[data-explorer]');
  if (explorer) {
    return {
      kind: 'dir',
      path: cleanRelPath(explorer.dataset.currentPath || ''),
    };
  }

  const file = qsel('.ghrm-page-shell[data-ghrm-view-kind]');
  if (file) {
    return {
      kind: 'file',
      path: cleanRelPath(file.dataset.currentPath || ''),
    };
  }

  return null;
}

function dispatchLiveEvent(event: LiveEvent): void {
  const detail = { name: event.name, path: event.path };
  document.dispatchEvent(new CustomEvent('ghrm:live', { detail }));
  document.dispatchEvent(
    new CustomEvent(`ghrm:live:${event.name}`, { detail }),
  );
}

export async function preserveEditedFile(shell: HTMLElement): Promise<boolean> {
  try {
    const current = await readEditVersion(shell);
    if (current === shell.dataset.ghrmEditVersion) {
      return true;
    }
    if (shell.dataset.ghrmEditDirty === '1') {
      shell.dataset.ghrmEditConflict = '1';
      shell.dataset.ghrmEditConflictVersion = current;
      return true;
    }
    return false;
  } catch {
    if (shell.dataset.ghrmEditDirty === '1') {
      shell.dataset.ghrmEditConflict = '1';
      delete shell.dataset.ghrmEditConflictVersion;
      return true;
    }
    return false;
  }
}

async function reloadAfterReconnect(): Promise<void> {
  const current = currentContentPath();
  if (!current) return;
  const shell = editingShell();
  if (current.kind === 'file' && shell && (await preserveEditedFile(shell))) {
    return;
  }
  location.reload();
}

async function handleLiveEvent(message: string): Promise<void> {
  const event = parseLiveMessage(message);
  const current = currentContentPath();
  const shell = editingShell();
  const reloadsCurrent =
    event.name === 'reload' && shouldReloadForChange(current, event.path);
  if (
    event.name === 'reload' &&
    !reloadsCurrent &&
    !shouldNavigateToParent(current, event.path)
  ) {
    return;
  }
  if (
    reloadsCurrent &&
    current?.kind === 'file' &&
    shell &&
    (await preserveEditedFile(shell))
  ) {
    return;
  }

  dispatchLiveEvent(event);
  if (event.name === 'reload' && shouldNavigateToParent(current, event.path)) {
    location.assign(parentHref(event.path || ''));
  } else if (event.name === 'reload') {
    location.reload();
  } else if (event.name === 'nav-ready') {
    refreshActiveSearch();
  }
}

function stripLeadingSlashes(path: string): string {
  let start = 0;
  while (path[start] === '/') {
    start += 1;
  }
  return path.slice(start);
}

function stripTrailingSlashes(path: string): string {
  let end = path.length;
  while (end > 0 && path[end - 1] === '/') {
    end -= 1;
  }
  return path.slice(0, end);
}

function parentPath(path: string): string {
  const clean = cleanRelPath(path);
  const slash = clean.lastIndexOf('/');
  return slash === -1 ? '' : clean.slice(0, slash);
}

function parentHref(path: string): string {
  const parent = parentPath(path);
  const href = parent === '' ? '/' : `/${encodePath(parent)}/`;
  return `${href}${location.search}${location.hash}`;
}

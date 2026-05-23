import { qsel } from './dom';
import { refreshActiveSearch } from './search';
import { setConnected } from './status';

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
        if (currentContentPath()) {
          location.reload();
        }
        return;
      }
      connectedOnce = true;
    };
    ws.onmessage = (ev) => {
      handleLiveEvent(ev.data);
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

function handleLiveEvent(message: string): void {
  const event = parseLiveMessage(message);
  if (
    event.name === 'reload' &&
    !shouldReloadForChange(currentContentPath(), event.path)
  ) {
    return;
  }

  dispatchLiveEvent(event);
  if (event.name === 'reload') {
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

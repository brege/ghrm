export function parseLiveMessage(message) {
  const reloadPrefix = 'reload:';
  if (message.startsWith(reloadPrefix)) {
    return {
      name: 'reload',
      path: decodeURIComponent(message.slice(reloadPrefix.length)),
    };
  }
  return { name: message, path: null };
}

export function cleanRelPath(path) {
  return stripTrailingSlashes(stripLeadingSlashes(path));
}

export function shouldReloadForChange(current, path) {
  if (!current) return path === null;
  if (path === null) return true;

  const changed = cleanRelPath(path);
  if (!changed) return false;
  if (current.kind === 'file') return changed === current.path;
  return parentPath(changed) === current.path;
}

function stripLeadingSlashes(path) {
  let start = 0;
  while (path[start] === '/') {
    start += 1;
  }
  return path.slice(start);
}

function stripTrailingSlashes(path) {
  let end = path.length;
  while (end > 0 && path[end - 1] === '/') {
    end -= 1;
  }
  return path.slice(0, end);
}

function parentPath(path) {
  const clean = cleanRelPath(path);
  const slash = clean.lastIndexOf('/');
  return slash === -1 ? '' : clean.slice(0, slash);
}

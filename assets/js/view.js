export function defaultShowHidden() {
  return document.body?.dataset.defaultShowHidden === '1';
}

export function defaultShowExcludes() {
  return document.body?.dataset.defaultShowExcludes === '1';
}

export function defaultUseIgnore() {
  return document.body?.dataset.defaultUseIgnore === '1';
}

export function defaultFilterExt() {
  return document.body?.dataset.defaultFilterExt === '1';
}

export function defaultFilterGroup() {
  return document.body?.dataset.defaultFilterGroup || null;
}

export function defaultFilterGroups() {
  const group = defaultFilterGroup();
  return group ? [group] : [];
}

export function defaultSort() {
  return document.body?.dataset.defaultSort || 'name';
}

export function defaultSortDir(sort = defaultSort()) {
  return sort === 'timestamp' ? 'desc' : 'asc';
}

export function defaultShowDate() {
  return document.body?.dataset.defaultShowDate === '1';
}

export function defaultShowCommit() {
  return document.body?.dataset.defaultShowCommit === '1';
}

export function canToggleExcludes() {
  return document.body?.dataset.canToggleExcludes === '1';
}

function parseQueryBool(raw) {
  if (raw === '1' || raw === 'true') return true;
  if (raw === '0' || raw === 'false') return false;
  return null;
}

function parseSort(raw) {
  switch (raw) {
    case 'name':
    case 'type':
    case 'timestamp':
      return raw;
    default:
      return null;
  }
}

function parseSortDir(raw) {
  switch (raw) {
    case 'asc':
    case 'desc':
      return raw;
    default:
      return null;
  }
}

export function currentView() {
  const params = new URLSearchParams(location.search);
  const hasGroups = params.has('group');
  const groups = params.getAll('group').filter((group) => group);
  return {
    showHidden: parseQueryBool(params.get('hidden')) ?? defaultShowHidden(),
    showExcludes: canToggleExcludes()
      ? (parseQueryBool(params.get('excludes')) ?? defaultShowExcludes())
      : false,
    useIgnore: parseQueryBool(params.get('ignore')) ?? defaultUseIgnore(),
    filterExt: parseQueryBool(params.get('filter')) ?? defaultFilterExt(),
    sort: parseSort(params.get('sort')) || defaultSort(),
    sortDir:
      parseSortDir(params.get('dir')) ||
      defaultSortDir(parseSort(params.get('sort')) || defaultSort()),
    filterGroups: hasGroups ? [...new Set(groups)] : defaultFilterGroups(),
    showDate: parseQueryBool(params.get('date')) ?? defaultShowDate(),
    showCommit: parseQueryBool(params.get('commit')) ?? defaultShowCommit(),
  };
}

function setQueryBool(params, key, value, defaultValue) {
  if (value === defaultValue) {
    params.delete(key);
  } else {
    params.set(key, value ? '1' : '0');
  }
}

export function withView(urlLike, view = currentView()) {
  const url = new URL(urlLike, location.origin);
  setQueryBool(
    url.searchParams,
    'hidden',
    view.showHidden,
    defaultShowHidden(),
  );
  if (canToggleExcludes()) {
    setQueryBool(
      url.searchParams,
      'excludes',
      view.showExcludes,
      defaultShowExcludes(),
    );
  } else {
    url.searchParams.delete('excludes');
  }
  setQueryBool(url.searchParams, 'ignore', view.useIgnore, defaultUseIgnore());
  setQueryBool(url.searchParams, 'filter', view.filterExt, defaultFilterExt());
  setQueryBool(url.searchParams, 'date', view.showDate, defaultShowDate());
  setQueryBool(
    url.searchParams,
    'commit',
    view.showCommit,
    defaultShowCommit(),
  );
  if (view.sort === defaultSort()) {
    url.searchParams.delete('sort');
  } else {
    url.searchParams.set('sort', view.sort);
  }
  if (view.sortDir === defaultSortDir(view.sort)) {
    url.searchParams.delete('dir');
  } else {
    url.searchParams.set('dir', view.sortDir);
  }
  url.searchParams.delete('group');
  const groups = [...new Set(view.filterGroups)];
  const defaults = defaultFilterGroups();
  if (
    groups.length !== defaults.length ||
    groups.some((group, index) => group !== defaults[index])
  ) {
    if (groups.length === 0) {
      url.searchParams.append('group', '');
    }
    for (const group of groups) {
      url.searchParams.append('group', group);
    }
  }
  return `${url.pathname}${url.search}${url.hash}`;
}

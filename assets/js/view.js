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
  return sortDefs().find((def) => def.key === sort)?.defaultDir || 'asc';
}

export function canToggleExcludes() {
  return document.body?.dataset.canToggleExcludes === '1';
}

export function columnDefs() {
  try {
    const raw = JSON.parse(document.body?.dataset.columns || '[]');
    return Array.isArray(raw) ? raw : [];
  } catch {
    return [];
  }
}

export function columnKeys() {
  return columnDefs()
    .map((column) => column.key)
    .filter((key) => key);
}

export function sortDefs() {
  try {
    const raw = JSON.parse(document.body?.dataset.sorts || '[]');
    return Array.isArray(raw) ? raw : [];
  } catch {
    return [];
  }
}

export function sortColumnKey(sort) {
  return sortDefs().find((def) => def.key === sort)?.columnKey || null;
}

export function sortAvailable(sort, columns) {
  const column = sortColumnKey(sort);
  return !column || columns.has(column);
}

export function defaultColumns() {
  return new Set(
    columnDefs()
      .filter((column) => column.defaultVisible)
      .map((column) => column.key),
  );
}

export function hasEdgeColumn(keys) {
  return columnDefs().some((column) => column.edge && keys.has(column.key));
}

function parseQueryBool(raw) {
  if (raw === '1' || raw === 'true') return true;
  if (raw === '0' || raw === 'false') return false;
  return null;
}

function parseSort(raw) {
  return sortDefs().some((def) => def.key === raw) ? raw : null;
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
  const columns = defaultColumns();
  for (const key of columnKeys()) {
    const visible = parseQueryBool(params.get(key));
    if (visible === null) continue;
    if (visible) {
      columns.add(key);
    } else {
      columns.delete(key);
    }
  }
  const parsedSort = parseSort(params.get('sort')) || defaultSort();
  const sort = sortAvailable(parsedSort, columns) ? parsedSort : defaultSort();
  return {
    showHidden: parseQueryBool(params.get('hidden')) ?? defaultShowHidden(),
    showExcludes: canToggleExcludes()
      ? (parseQueryBool(params.get('excludes')) ?? defaultShowExcludes())
      : false,
    useIgnore: parseQueryBool(params.get('ignore')) ?? defaultUseIgnore(),
    filterExt: parseQueryBool(params.get('filter')) ?? defaultFilterExt(),
    sort,
    sortDir: parseSortDir(params.get('dir')) || defaultSortDir(sort),
    filterGroups: hasGroups ? [...new Set(groups)] : defaultFilterGroups(),
    columns,
    showHeaders: parseQueryBool(params.get('headers')) ?? false,
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
  const columnDefaults = defaultColumns();
  for (const key of columnKeys()) {
    setQueryBool(
      url.searchParams,
      key,
      view.columns.has(key),
      columnDefaults.has(key),
    );
  }
  setQueryBool(url.searchParams, 'headers', view.showHeaders, false);
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
  const groupDefaults = defaultFilterGroups();
  if (
    groups.length !== groupDefaults.length ||
    groups.some((group, index) => group !== groupDefaults[index])
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

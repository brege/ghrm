import { beforeEach, describe, expect, it } from 'vitest';
import { isPrintMode, syncPrintMode } from '../prefs';

function setSearch(search: string): void {
  window.history.pushState({}, '', `/${search}`);
}

function setFileView(): void {
  document.body.innerHTML =
    '<section class="ghrm-page-shell" data-ghrm-view-kind="markdown"></section>';
}

describe('print mode', () => {
  beforeEach(() => {
    localStorage.clear();
    document.body.innerHTML = '';
    document.body.className = '';
    setSearch('');
  });

  it('applies to file views from the print query', () => {
    setSearch('?print=1');
    setFileView();

    syncPrintMode();

    expect(isPrintMode()).toBe(true);
    expect(document.body.classList.contains('ghrm-print')).toBe(true);
    expect(document.body.classList.contains('ghrm-wrap')).toBe(true);
  });

  it('requires an explicit true print value', () => {
    setSearch('?print=');
    setFileView();

    syncPrintMode();

    expect(isPrintMode()).toBe(false);
    expect(document.body.classList.contains('ghrm-print')).toBe(false);
  });

  it('does not apply to explorer views', () => {
    setSearch('?print=1');
    document.body.innerHTML = '<article data-explorer="true"></article>';

    syncPrintMode();

    expect(isPrintMode()).toBe(false);
    expect(document.body.classList.contains('ghrm-print')).toBe(false);
  });

  it('does not persist view prefs', () => {
    setSearch('?print=1');
    setFileView();

    syncPrintMode();

    expect(localStorage.getItem('ghrm-doc-flat')).toBeNull();
    expect(localStorage.getItem('ghrm-wrap')).toBeNull();
  });
});

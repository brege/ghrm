import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  applyDocChromePref,
  isPrintMode,
  setupDocChromeToggle,
  syncPrintMode,
} from '../prefs';

function setUrl(path: string): void {
  window.history.pushState({}, '', path);
}

function setFileView(toggle = false): void {
  const button = toggle
    ? '<button id="doc-chrome-toggle" type="button" hidden></button>'
    : '';
  document.body.innerHTML = `${button}<section class="ghrm-page-shell" data-ghrm-view-kind="markdown"></section>`;
}

describe('print mode', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    localStorage.clear();
    document.body.innerHTML = '';
    document.body.className = '';
    setUrl('/');
  });

  it('applies to file views from the print query', () => {
    setUrl('/?print=1');
    setFileView();

    syncPrintMode();

    expect(isPrintMode()).toBe(true);
    expect(document.body.classList.contains('ghrm-print')).toBe(true);
    expect(document.body.classList.contains('ghrm-wrap')).toBe(true);
  });

  it('requires an explicit true print value', () => {
    setUrl('/?print=');
    setFileView();

    syncPrintMode();

    expect(isPrintMode()).toBe(false);
    expect(document.body.classList.contains('ghrm-print')).toBe(false);
  });

  it('does not apply to explorer views', () => {
    setUrl('/?print=1');
    document.body.innerHTML = '<article data-explorer="true"></article>';

    syncPrintMode();

    expect(isPrintMode()).toBe(false);
    expect(document.body.classList.contains('ghrm-print')).toBe(false);
  });

  it('does not persist view prefs', () => {
    setUrl('/?print=1');
    setFileView();

    syncPrintMode();

    expect(localStorage.getItem('ghrm-doc-flat')).toBeNull();
    expect(localStorage.getItem('ghrm-wrap')).toBeNull();
  });

  it('updates the document wrapper label from print mode', () => {
    setUrl('/docs/readme.md');
    setFileView(true);

    applyDocChromePref();

    const btn = document.getElementById('doc-chrome-toggle');
    expect(btn?.getAttribute('aria-label')).toBe('Hide document wrapper');

    setUrl('/docs/readme.md?print=1');
    syncPrintMode();
    applyDocChromePref();

    expect(btn?.getAttribute('aria-label')).toBe('Show document wrapper');
  });

  it('navigates to print mode through the query key', () => {
    const assign = vi
      .spyOn(window.location, 'assign')
      .mockImplementation(() => undefined);

    setUrl('/docs/readme.md?mode=raw#frag');
    setFileView(true);

    setupDocChromeToggle();
    document.getElementById('doc-chrome-toggle')?.click();

    expect(assign).toHaveBeenCalledWith(
      '/docs/readme.md?mode=raw&print=1#frag',
    );
    expect(localStorage.getItem('ghrm-doc-flat')).toBeNull();
  });

  it('removes the print query key when leaving print mode', () => {
    const assign = vi
      .spyOn(window.location, 'assign')
      .mockImplementation(() => undefined);

    setUrl('/docs/readme.md?mode=raw&print=1#frag');
    setFileView(true);

    setupDocChromeToggle();
    document.getElementById('doc-chrome-toggle')?.click();

    expect(assign).toHaveBeenCalledWith('/docs/readme.md?mode=raw#frag');
  });
});

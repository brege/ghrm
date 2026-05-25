import { afterEach, describe, expect, it, vi } from 'vitest';

describe('main entrypoint', () => {
  afterEach(() => {
    vi.doUnmock('../features');
    vi.doUnmock('../runtime');
    vi.resetModules();
  });

  it('registers islands and starts browser runtime on DOMContentLoaded', async () => {
    const registerBrowserFeatures = vi.fn();
    const runInitial = vi.fn();

    vi.resetModules();
    vi.doMock('../features', () => ({ registerBrowserFeatures }));
    vi.doMock('../runtime', () => ({ runInitial }));

    await import('../main');
    document.dispatchEvent(new Event('DOMContentLoaded'));

    expect(customElements.get('ghrm-explorer-menus')).toBeDefined();
    expect(customElements.get('ghrm-archive-progress')).toBeDefined();
    expect(customElements.get('ghrm-search-panel')).toBeDefined();
    expect(customElements.get('ghrm-gist-editor')).toBeDefined();
    expect(customElements.get('ghrm-gist-stash')).toBeDefined();
    expect(registerBrowserFeatures).toHaveBeenCalledOnce();
    expect(runInitial).toHaveBeenCalledOnce();
  });
});

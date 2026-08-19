import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  preserveEditedFile,
  shouldNavigateToParent,
  shouldReloadForChange,
} from '../live';

afterEach(() => {
  document.body.innerHTML = '';
  vi.restoreAllMocks();
});

describe('live reload filtering', () => {
  it('reloads directory views only for direct children', () => {
    expect(shouldReloadForChange({ kind: 'dir', path: '' }, 'docs')).toBe(true);
    expect(
      shouldReloadForChange({ kind: 'dir', path: '' }, 'docs/api/index.md'),
    ).toBe(false);
    expect(
      shouldReloadForChange({ kind: 'dir', path: 'docs' }, 'docs/index.md'),
    ).toBe(true);
    expect(
      shouldReloadForChange(
        { kind: 'dir', path: 'docs' },
        'docs/archive/old.md',
      ),
    ).toBe(false);
  });

  it('navigates up when the active directory is removed', () => {
    expect(
      shouldNavigateToParent(
        { kind: 'dir', path: 'mutants.out' },
        'mutants.out',
      ),
    ).toBe(true);
    expect(
      shouldNavigateToParent({ kind: 'dir', path: 'docs/api' }, 'docs'),
    ).toBe(true);
  });

  it('does not navigate away from file reloads', () => {
    expect(
      shouldNavigateToParent(
        { kind: 'file', path: 'docs/index.md' },
        'docs/index.md',
      ),
    ).toBe(false);
  });

  it('suppresses only reloads whose disk version matches the editor', async () => {
    document.body.innerHTML = `
      <section data-ghrm-raw-url="/_ghrm/raw/notes.md"
        data-ghrm-edit-version="saved-version"></section>`;
    const shell = document.querySelector<HTMLElement>('section')!;
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(null, {
        status: 200,
        headers: { ETag: '"saved-version"' },
      }),
    );

    expect(await preserveEditedFile(shell)).toBe(true);
    expect(fetchSpy.mock.calls[0][0]).toBe('/_ghrm/edit/notes.md');
    expect(fetchSpy.mock.calls[0][1]?.method).toBe('HEAD');

    fetchSpy.mockResolvedValue(
      new Response(null, {
        status: 200,
        headers: { ETag: '"external-version"' },
      }),
    );
    shell.dataset.ghrmEditDirty = '1';

    expect(await preserveEditedFile(shell)).toBe(true);
    expect(shell.dataset.ghrmEditConflict).toBe('1');
    expect(shell.dataset.ghrmEditConflictVersion).toBe('external-version');
  });

  it('allows a clean editor to reload for a changed disk version', async () => {
    document.body.innerHTML = `
      <section data-ghrm-raw-url="/_ghrm/raw/notes.md"
        data-ghrm-edit-version="saved-version"></section>`;
    const shell = document.querySelector<HTMLElement>('section')!;
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(null, {
        status: 200,
        headers: { ETag: '"external-version"' },
      }),
    );

    expect(await preserveEditedFile(shell)).toBe(false);
    expect(shell.dataset.ghrmEditConflict).toBeUndefined();
  });
});

import { describe, expect, it } from 'vitest';
import { shouldNavigateToParent, shouldReloadForChange } from '../live';

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
});

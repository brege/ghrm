import { describe, expect, it } from 'vitest';
import { browserFeatures } from '../features';
import type { LifecyclePhase } from '../runtime';

function featureNames(phase: LifecyclePhase): string[] {
  return browserFeatures
    .filter((feature) => feature.phase === phase)
    .sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
    .map((feature) => feature.name);
}

describe('browser features', () => {
  it('declares initial setup order', () => {
    expect(featureNames('initial')).toEqual([
      'file-views',
      'search-close',
      'view-menu',
      'doc-chrome-toggle',
      'dates',
      'toc',
      'nav-links',
      'theme-toggle',
      'status-peek',
      'live-reload',
      'htmx-nav',
      'hash-scroll',
    ]);
  });

  it('declares refresh setup order', () => {
    expect(featureNames('refresh')).toEqual([
      'server-status',
      'file-views',
      'nav-links',
      'view-menu',
      'column-controls',
      'doc-chrome-pref',
      'dates',
      'toc-build',
    ]);
  });
});

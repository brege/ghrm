import { scrollToHash } from './dom';
import {
  populateDates,
  setupNavExternalLinks,
  syncColumnControls,
} from './explorer';
import { setupFileViews } from './file';
import { setupLiveReload } from './live';
import { setupHtmxNav } from './nav';
import {
  applyDocChromePref,
  setupDocChromeToggle,
  setupThemeToggle,
} from './prefs';
import { type FeatureEntry, registerFeatures } from './runtime';
import { setSearchCloseHandler } from './search';
import { setupStatusPeek, syncServerStatus } from './status';
import { buildToc, setupToc } from './toc';

function setupSearchClose(): void {
  setSearchCloseHandler(() => {
    const target = `${location.pathname}${location.search}${location.hash}`;
    location.assign(target);
  });
}

export const browserFeatures: readonly FeatureEntry[] = [
  { name: 'file-views', phase: 'initial', order: 100, setup: setupFileViews },
  {
    name: 'search-close',
    phase: 'initial',
    order: 105,
    setup: setupSearchClose,
  },
  {
    name: 'column-controls',
    phase: 'initial',
    order: 120,
    setup: syncColumnControls,
  },
  {
    name: 'doc-chrome-toggle',
    phase: 'initial',
    order: 130,
    setup: setupDocChromeToggle,
  },
  { name: 'dates', phase: 'initial', order: 140, setup: populateDates },
  { name: 'toc', phase: 'initial', order: 150, setup: setupToc },
  {
    name: 'nav-links',
    phase: 'initial',
    order: 160,
    setup: setupNavExternalLinks,
  },
  {
    name: 'theme-toggle',
    phase: 'initial',
    order: 200,
    setup: setupThemeToggle,
  },
  { name: 'status-peek', phase: 'initial', order: 210, setup: setupStatusPeek },
  { name: 'live-reload', phase: 'initial', order: 220, setup: setupLiveReload },
  { name: 'htmx-nav', phase: 'initial', order: 230, setup: setupHtmxNav },
  {
    name: 'hash-scroll',
    phase: 'initial',
    order: 240,
    setup: () => scrollToHash(location.hash),
  },
  {
    name: 'server-status',
    phase: 'refresh',
    order: 100,
    setup: syncServerStatus,
  },
  { name: 'file-views', phase: 'refresh', order: 110, setup: setupFileViews },
  {
    name: 'nav-links',
    phase: 'refresh',
    order: 130,
    setup: setupNavExternalLinks,
  },
  {
    name: 'column-controls',
    phase: 'refresh',
    order: 140,
    setup: syncColumnControls,
  },
  {
    name: 'doc-chrome-pref',
    phase: 'refresh',
    order: 160,
    setup: applyDocChromePref,
  },
  { name: 'dates', phase: 'refresh', order: 170, setup: populateDates },
  { name: 'toc-build', phase: 'refresh', order: 180, setup: buildToc },
];

export function registerBrowserFeatures(): void {
  registerFeatures(browserFeatures);
}

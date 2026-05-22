import { scrollToHash } from './dom.js';
import { setupLiveReload } from './live.js';
import { setupHtmxNav, setupInitialContent } from './nav.js';
import { setupThemeToggle } from './prefs.js';
import { setupStatusPeek } from './status.js';

document.addEventListener('DOMContentLoaded', () => {
  setupInitialContent();
  setupThemeToggle();
  setupStatusPeek();
  setupLiveReload();
  setupHtmxNav();
  scrollToHash(location.hash);
});

import { scrollToHash } from './dom';
import { setupLiveReload } from './live';
import { setupHtmxNav, setupInitialContent } from './nav';
import { setupThemeToggle } from './prefs';
import { setupStatusPeek } from './status';

document.addEventListener('DOMContentLoaded', () => {
  setupInitialContent();
  setupThemeToggle();
  setupStatusPeek();
  setupLiveReload();
  setupHtmxNav();
  scrollToHash(location.hash);
});

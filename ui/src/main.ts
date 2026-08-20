import { registerBrowserFeatures } from './features';
import './archive/progress';
import './shell/menu';
import './search/panel';
import './gist/editor';
import './gist/stash';
import { runInitial } from './runtime';

document.addEventListener('DOMContentLoaded', () => {
  registerBrowserFeatures();
  runInitial();
});

import { registerBrowserFeatures } from './features';
import './islands/define';
import { runInitial } from './runtime';

document.addEventListener('DOMContentLoaded', () => {
  registerBrowserFeatures();
  runInitial();
});

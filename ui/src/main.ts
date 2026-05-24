import { registerBrowserFeatures } from './features';
import { runInitial } from './runtime';

document.addEventListener('DOMContentLoaded', () => {
  registerBrowserFeatures();
  runInitial();
});

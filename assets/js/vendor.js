const vendorLoading = new Map();
let assetConfig;

function loadScript(src) {
  if (vendorLoading.has(src)) return vendorLoading.get(src);
  if (document.querySelector(`script[src="${src}"]`)) return Promise.resolve();
  const promise = new Promise((resolve, reject) => {
    const script = document.createElement('script');
    script.src = src;
    script.onload = resolve;
    script.onerror = reject;
    document.head.appendChild(script);
  });
  vendorLoading.set(src, promise);
  return promise;
}

function loadStylesheet(href) {
  if (document.querySelector(`link[href="${href}"]`)) return;
  const link = document.createElement('link');
  link.rel = 'stylesheet';
  link.href = href;
  document.head.appendChild(link);
}

function currentArticle() {
  return document.querySelector('article.markdown-body');
}

export function assetPlan() {
  if (!assetConfig) {
    assetConfig = JSON.parse(
      document.getElementById('ghrm-assets')?.textContent || '{"features":{}}',
    );
  }
  return assetConfig;
}

function currentFeatures() {
  return (currentArticle()?.dataset.ghrmFeatures || '')
    .split(/\s+/)
    .filter((value) => value);
}

export function hasFeature(name) {
  return currentFeatures().includes(name);
}

export async function loadAssets() {
  const config = assetPlan();
  for (const name of currentFeatures()) {
    const feature = config.features?.[name];
    if (!feature) continue;
    for (const href of feature.styles || []) loadStylesheet(href);
    for (const src of feature.scripts || []) await loadScript(src);
  }
}

import { qsel } from './dom';

interface FeatureAssets {
  styles?: string[];
  scripts?: string[];
}

interface AssetConfig {
  features: Record<string, FeatureAssets>;
  mermaidVersion?: string;
}

const vendorLoading = new Map<string, Promise<void>>();
let assetConfig: AssetConfig | undefined;

function loadScript(src: string): Promise<void> {
  const cached = vendorLoading.get(src);
  if (cached) return cached;
  if (document.querySelector(`script[src="${src}"]`)) return Promise.resolve();
  const promise = new Promise<void>((resolve, reject) => {
    const script = document.createElement('script');
    script.src = src;
    script.onload = () => resolve();
    script.onerror = reject;
    document.head.appendChild(script);
  });
  vendorLoading.set(src, promise);
  return promise;
}

function loadStylesheet(href: string): void {
  if (document.querySelector(`link[href="${href}"]`)) return;
  const link = document.createElement('link');
  link.rel = 'stylesheet';
  link.href = href;
  document.head.appendChild(link);
}

function currentArticle(): HTMLElement | null {
  return qsel('article.markdown-body');
}

export function assetPlan(): AssetConfig {
  if (!assetConfig) {
    assetConfig = JSON.parse(
      document.getElementById('ghrm-assets')?.textContent || '{"features":{}}',
    );
  }
  return assetConfig!;
}

function currentFeatures(): string[] {
  const article = currentArticle();
  return (article?.dataset.ghrmFeatures || '')
    .split(/\s+/)
    .filter((value) => value);
}

export function hasFeature(name: string): boolean {
  return currentFeatures().includes(name);
}

export async function loadAssets(): Promise<void> {
  const config = assetPlan();
  for (const name of currentFeatures()) {
    const feature = config.features?.[name];
    if (!feature) continue;
    for (const href of feature.styles || []) loadStylesheet(href);
    for (const src of feature.scripts || []) await loadScript(src);
  }
}

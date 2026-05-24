export type LifecyclePhase = 'initial' | 'refresh';

export interface FeatureEntry {
  name: string;
  phase: LifecyclePhase;
  setup: () => void;
  order?: number;
}

const features: FeatureEntry[] = [];
let initialized = false;

export function registerFeature(entry: FeatureEntry): void {
  features.push(entry);
}

export function runPhase(phase: LifecyclePhase): void {
  const entries = features
    .filter((f) => f.phase === phase)
    .sort((a, b) => (a.order ?? 0) - (b.order ?? 0));
  for (const entry of entries) {
    entry.setup();
  }
}

export function runInitial(): void {
  if (initialized) return;
  initialized = true;
  runPhase('initial');
}

export function runRefresh(): void {
  runPhase('refresh');
}

export function isInitialized(): boolean {
  return initialized;
}

export function resetRuntime(): void {
  features.length = 0;
  initialized = false;
}

export function getFeatureNames(phase?: LifecyclePhase): string[] {
  const entries = phase ? features.filter((f) => f.phase === phase) : features;
  return [...entries]
    .sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
    .map((f) => f.name);
}

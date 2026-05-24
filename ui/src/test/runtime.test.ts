import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  getFeatureNames,
  isInitialized,
  registerFeature,
  resetRuntime,
  runInitial,
  runPhase,
  runRefresh,
} from '../runtime';

describe('runtime', () => {
  beforeEach(() => {
    resetRuntime();
    document.body.innerHTML = '';
  });

  describe('registerFeature', () => {
    it('adds feature to registry', () => {
      registerFeature({ name: 'test', phase: 'initial', setup: () => {} });
      expect(getFeatureNames()).toContain('test');
    });

    it('supports multiple features', () => {
      registerFeature({ name: 'a', phase: 'initial', setup: () => {} });
      registerFeature({ name: 'b', phase: 'refresh', setup: () => {} });
      expect(getFeatureNames()).toEqual(['a', 'b']);
    });
  });

  describe('runInitial', () => {
    it('runs initial phase features', () => {
      const setup = vi.fn();
      registerFeature({ name: 'test', phase: 'initial', setup });
      runInitial();
      expect(setup).toHaveBeenCalledOnce();
    });

    it('sets initialized flag', () => {
      expect(isInitialized()).toBe(false);
      runInitial();
      expect(isInitialized()).toBe(true);
    });

    it('only runs once', () => {
      const setup = vi.fn();
      registerFeature({ name: 'test', phase: 'initial', setup });
      runInitial();
      runInitial();
      expect(setup).toHaveBeenCalledOnce();
    });

    it('does not run refresh features', () => {
      const initial = vi.fn();
      const refresh = vi.fn();
      registerFeature({ name: 'a', phase: 'initial', setup: initial });
      registerFeature({ name: 'b', phase: 'refresh', setup: refresh });
      runInitial();
      expect(initial).toHaveBeenCalledOnce();
      expect(refresh).not.toHaveBeenCalled();
    });
  });

  describe('runRefresh', () => {
    it('runs refresh phase features', () => {
      const setup = vi.fn();
      registerFeature({ name: 'test', phase: 'refresh', setup });
      runRefresh();
      expect(setup).toHaveBeenCalledOnce();
    });

    it('can run multiple times', () => {
      const setup = vi.fn();
      registerFeature({ name: 'test', phase: 'refresh', setup });
      runRefresh();
      runRefresh();
      expect(setup).toHaveBeenCalledTimes(2);
    });

    it('does not run initial features', () => {
      const initial = vi.fn();
      const refresh = vi.fn();
      registerFeature({ name: 'a', phase: 'initial', setup: initial });
      registerFeature({ name: 'b', phase: 'refresh', setup: refresh });
      runRefresh();
      expect(refresh).toHaveBeenCalledOnce();
      expect(initial).not.toHaveBeenCalled();
    });
  });

  describe('feature ordering', () => {
    it('runs features in order by default', () => {
      const order: string[] = [];
      registerFeature({
        name: 'a',
        phase: 'initial',
        setup: () => order.push('a'),
      });
      registerFeature({
        name: 'b',
        phase: 'initial',
        setup: () => order.push('b'),
      });
      runInitial();
      expect(order).toEqual(['a', 'b']);
    });

    it('respects explicit order', () => {
      const order: string[] = [];
      registerFeature({
        name: 'a',
        phase: 'initial',
        setup: () => order.push('a'),
        order: 10,
      });
      registerFeature({
        name: 'b',
        phase: 'initial',
        setup: () => order.push('b'),
        order: 5,
      });
      registerFeature({
        name: 'c',
        phase: 'initial',
        setup: () => order.push('c'),
        order: 15,
      });
      runInitial();
      expect(order).toEqual(['b', 'a', 'c']);
    });

    it('treats missing order as zero', () => {
      const order: string[] = [];
      registerFeature({
        name: 'a',
        phase: 'initial',
        setup: () => order.push('a'),
        order: 5,
      });
      registerFeature({
        name: 'b',
        phase: 'initial',
        setup: () => order.push('b'),
      });
      runInitial();
      expect(order).toEqual(['b', 'a']);
    });
  });

  describe('getFeatureNames', () => {
    it('returns all feature names', () => {
      registerFeature({ name: 'x', phase: 'initial', setup: () => {} });
      registerFeature({ name: 'y', phase: 'refresh', setup: () => {} });
      expect(getFeatureNames()).toEqual(['x', 'y']);
    });

    it('filters by phase', () => {
      registerFeature({ name: 'x', phase: 'initial', setup: () => {} });
      registerFeature({ name: 'y', phase: 'refresh', setup: () => {} });
      expect(getFeatureNames('initial')).toEqual(['x']);
      expect(getFeatureNames('refresh')).toEqual(['y']);
    });

    it('returns names in order', () => {
      registerFeature({
        name: 'b',
        phase: 'initial',
        setup: () => {},
        order: 10,
      });
      registerFeature({
        name: 'a',
        phase: 'initial',
        setup: () => {},
        order: 5,
      });
      expect(getFeatureNames('initial')).toEqual(['a', 'b']);
    });
  });

  describe('resetRuntime', () => {
    it('clears features', () => {
      registerFeature({ name: 'test', phase: 'initial', setup: () => {} });
      resetRuntime();
      expect(getFeatureNames()).toEqual([]);
    });

    it('clears initialized flag', () => {
      runInitial();
      expect(isInitialized()).toBe(true);
      resetRuntime();
      expect(isInitialized()).toBe(false);
    });

    it('allows runInitial to run again', () => {
      const setup = vi.fn();
      registerFeature({ name: 'test', phase: 'initial', setup });
      runInitial();
      resetRuntime();
      registerFeature({ name: 'test', phase: 'initial', setup });
      runInitial();
      expect(setup).toHaveBeenCalledTimes(2);
    });
  });

  describe('runPhase', () => {
    it('runs specific phase directly', () => {
      const initial = vi.fn();
      const refresh = vi.fn();
      registerFeature({ name: 'a', phase: 'initial', setup: initial });
      registerFeature({ name: 'b', phase: 'refresh', setup: refresh });
      runPhase('refresh');
      expect(refresh).toHaveBeenCalledOnce();
      expect(initial).not.toHaveBeenCalled();
    });
  });

  describe('dom setup', () => {
    it('runs initial setup against document content', () => {
      document.body.innerHTML = '<button data-ghrm-feature>Off</button>';
      registerFeature({
        name: 'button-label',
        phase: 'initial',
        setup: () => {
          const button = document.querySelector<HTMLButtonElement>(
            '[data-ghrm-feature]',
          );
          if (!button) {
            throw new Error('missing feature button');
          }
          button.textContent = 'On';
        },
      });

      runInitial();

      expect(document.querySelector('[data-ghrm-feature]')?.textContent).toBe(
        'On',
      );
    });

    it('runs refresh setup against replaced document content', () => {
      const seen: string[] = [];
      registerFeature({
        name: 'refresh-label',
        phase: 'refresh',
        setup: () => {
          const label = document.querySelector('[data-ghrm-label]');
          if (!label?.textContent) {
            throw new Error('missing refresh label');
          }
          seen.push(label.textContent);
        },
      });

      document.body.innerHTML = '<p data-ghrm-label>first</p>';
      runRefresh();
      document.body.innerHTML = '<p data-ghrm-label>second</p>';
      runRefresh();

      expect(seen).toEqual(['first', 'second']);
    });
  });
});

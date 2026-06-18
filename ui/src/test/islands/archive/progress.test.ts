import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '../../../islands/archive/progress';
import type { GhrmArchiveProgress } from '../../../islands/archive/progress';

describe('ghrm-archive-progress', () => {
  let element: GhrmArchiveProgress;

  function jsonResponse(body: unknown): Response {
    return new Response(JSON.stringify(body), { status: 200 });
  }

  function requestUrl(url: Parameters<typeof fetch>[0]): string {
    return typeof url === 'string' ? url : url.toString();
  }

  beforeEach(() => {
    document.body.innerHTML = '<ghrm-archive-progress></ghrm-archive-progress>';
    const found = document.querySelector<GhrmArchiveProgress>(
      'ghrm-archive-progress',
    );
    if (!found) {
      throw new Error('missing ghrm-archive-progress');
    }
    element = found;
  });

  afterEach(() => {
    vi.useRealTimers();
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  describe('initial state', () => {
    it('renders empty initially', () => {
      expect(element.querySelector('.ghrm-archive-progress')).toBeNull();
    });
  });

  describe('job start', () => {
    it('sends POST request with JSON accept header', async () => {
      const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
        jsonResponse({
          download_url: '/_ghrm/archive/test.zip',
          status_url: '/_ghrm/archive/status/123',
        }),
      );

      await element.startJob('/_ghrm/archive/start');

      expect(fetchSpy).toHaveBeenCalledWith('/_ghrm/archive/start', {
        method: 'POST',
        headers: { Accept: 'application/json' },
      });
    });

    it('shows progress container after startJob', async () => {
      vi.spyOn(globalThis, 'fetch').mockResolvedValue(
        jsonResponse({
          download_url: '/_ghrm/archive/test.zip',
          status_url: '/_ghrm/archive/status/123',
        }),
      );

      await element.startJob('/_ghrm/archive/start');
      await element.updateComplete;

      const container = element.querySelector('.ghrm-archive-progress');
      expect(container).toBeTruthy();
    });

    it('renders running state attribute', async () => {
      vi.spyOn(globalThis, 'fetch').mockImplementation(async (url) => {
        if (requestUrl(url).includes('start')) {
          return jsonResponse({
            download_url: '/_ghrm/archive/test.zip',
            status_url: '/_ghrm/archive/status/123',
          });
        }
        return jsonResponse({ state: 'running', percent: 10 });
      });

      await element.startJob('/_ghrm/archive/start');
      await element.updateComplete;

      const container = element.querySelector('.ghrm-archive-progress');
      expect(container?.getAttribute('data-state')).toBe('running');
    });
  });

  describe('status polling', () => {
    it('polls the returned status URL', async () => {
      const fetchSpy = vi
        .spyOn(globalThis, 'fetch')
        .mockImplementation(async (url) => {
          if (requestUrl(url).includes('start')) {
            return jsonResponse({
              download_url: '/_ghrm/archive/test.zip',
              status_url: '/_ghrm/archive/status/abc123',
            });
          }
          return jsonResponse({ state: 'complete', percent: 100 });
        });

      await element.startJob('/_ghrm/archive/start');
      await element.updateComplete;

      const statusCalls = fetchSpy.mock.calls.filter(([url]) =>
        requestUrl(url).includes('status/abc123'),
      );
      expect(statusCalls.length).toBeGreaterThan(0);
      expect(statusCalls[0][1]).toEqual({
        headers: { Accept: 'application/json' },
      });
    });

    it('updates rendered count text from status', async () => {
      vi.spyOn(globalThis, 'fetch').mockImplementation(async (url) => {
        if (requestUrl(url).includes('start')) {
          return jsonResponse({
            download_url: '/_ghrm/archive/test.zip',
            status_url: '/_ghrm/archive/status/123',
          });
        }
        return jsonResponse({
          state: 'running',
          percent: 42,
          done_files: 10,
          total_files: 25,
        });
      });

      await element.startJob('/_ghrm/archive/start');
      await element.updateComplete;

      const countEl = element.querySelector('.ghrm-archive-progress-count');
      expect(countEl?.textContent).toContain('42%');
      expect(countEl?.textContent).toContain('10 / 25 files');
    });

    it('updates fill width from percent', async () => {
      vi.spyOn(globalThis, 'fetch').mockImplementation(async (url) => {
        if (requestUrl(url).includes('start')) {
          return jsonResponse({
            download_url: '/_ghrm/archive/test.zip',
            status_url: '/_ghrm/archive/status/123',
          });
        }
        return jsonResponse({ state: 'running', percent: 65 });
      });

      await element.startJob('/_ghrm/archive/start');
      await element.updateComplete;

      const fillEl = element.querySelector(
        '.ghrm-archive-progress-fill',
      ) as HTMLElement;
      expect(fillEl?.style.width).toBe('65%');
    });
  });

  describe('completion', () => {
    it('renders complete state attribute', async () => {
      vi.spyOn(globalThis, 'fetch').mockImplementation(async (url) => {
        if (requestUrl(url).includes('start')) {
          return jsonResponse({
            download_url: '/_ghrm/archive/test.zip',
            status_url: '/_ghrm/archive/status/123',
          });
        }
        return jsonResponse({ state: 'complete', percent: 100 });
      });

      await element.startJob('/_ghrm/archive/start');
      await element.updateComplete;

      const container = element.querySelector('.ghrm-archive-progress');
      expect(container?.getAttribute('data-state')).toBe('complete');
    });

    it('renders complete label', async () => {
      vi.spyOn(globalThis, 'fetch').mockImplementation(async (url) => {
        if (requestUrl(url).includes('start')) {
          return jsonResponse({
            download_url: '/_ghrm/archive/test.zip',
            status_url: '/_ghrm/archive/status/123',
          });
        }
        return jsonResponse({ state: 'complete', percent: 100 });
      });

      await element.startJob('/_ghrm/archive/start');
      await element.updateComplete;

      const labelEl = element.querySelector('.ghrm-archive-progress-label');
      expect(labelEl?.textContent).toBe('Archive complete');
    });

    it('shows close button on complete and hides on click', async () => {
      vi.spyOn(globalThis, 'fetch').mockImplementation(async (url) => {
        if (requestUrl(url).includes('start')) {
          return jsonResponse({
            download_url: '/_ghrm/archive/test.zip',
            status_url: '/_ghrm/archive/status/123',
          });
        }
        return jsonResponse({ state: 'complete', percent: 100 });
      });

      await element.startJob('/_ghrm/archive/start');
      await element.updateComplete;
      expect(element.querySelector('.ghrm-archive-progress')).toBeTruthy();

      const closeBtn = element.querySelector<HTMLButtonElement>(
        '.ghrm-archive-progress-close',
      );
      expect(closeBtn).toBeTruthy();

      closeBtn?.click();
      await element.updateComplete;
      expect(element.querySelector('.ghrm-archive-progress')).toBeNull();
    });
  });

  describe('download trigger', () => {
    it('creates download link with correct href and download attribute', async () => {
      const appendedLinks: HTMLAnchorElement[] = [];
      const originalAppend = document.body.append.bind(document.body);
      vi.spyOn(document.body, 'append').mockImplementation((node) => {
        if (node instanceof HTMLAnchorElement) {
          appendedLinks.push(node);
        }
        return originalAppend(node);
      });

      vi.spyOn(globalThis, 'fetch').mockResolvedValue(
        jsonResponse({
          download_url: '/_ghrm/archive/test.zip',
          status_url: '/_ghrm/archive/status/123',
        }),
      );

      await element.startJob('/_ghrm/archive/start');
      await element.updateComplete;

      const downloadLink = appendedLinks.find(
        (l) => l.href.includes('test.zip') && l.hasAttribute('download'),
      );
      expect(downloadLink).toBeDefined();
      expect(downloadLink?.dataset.ghrmNative).toBe('1');
    });
  });

  describe('error handling', () => {
    it('renders failed state on network error', async () => {
      vi.spyOn(globalThis, 'fetch').mockRejectedValue(
        new Error('Network error'),
      );

      await element.startJob('/_ghrm/archive/start');
      await element.updateComplete;
      await vi.waitFor(
        () => element.querySelector('[data-state="failed"]') !== null,
      );

      const container = element.querySelector('.ghrm-archive-progress');
      expect(container?.getAttribute('data-state')).toBe('failed');

      const labelEl = element.querySelector('.ghrm-archive-progress-label');
      expect(labelEl?.textContent).toBe('Archive failed');
    });
  });

  describe('lifecycle', () => {
    it('clears timers on disconnect', async () => {
      vi.spyOn(globalThis, 'fetch').mockImplementation(async (url) => {
        if (requestUrl(url).includes('start')) {
          return jsonResponse({
            download_url: '/_ghrm/archive/test.zip',
            status_url: '/_ghrm/archive/status/123',
          });
        }
        return jsonResponse({ state: 'running', percent: 50 });
      });

      await element.startJob('/_ghrm/archive/start');
      await element.updateComplete;

      const clearTimeoutSpy = vi.spyOn(globalThis, 'clearTimeout');
      element.remove();

      expect(clearTimeoutSpy).toHaveBeenCalled();
    });

    it('does not duplicate timers on repeated startJob', async () => {
      vi.spyOn(globalThis, 'fetch').mockImplementation(async (url) => {
        if (requestUrl(url).includes('start')) {
          return jsonResponse({
            download_url: '/_ghrm/archive/test.zip',
            status_url: '/_ghrm/archive/status/123',
          });
        }
        return jsonResponse({ state: 'running', percent: 50 });
      });

      const clearTimeoutSpy = vi.spyOn(globalThis, 'clearTimeout');

      await element.startJob('/_ghrm/archive/start');
      await element.updateComplete;

      await element.startJob('/_ghrm/archive/start2');
      await element.updateComplete;

      expect(clearTimeoutSpy).toHaveBeenCalled();
    });
  });
});

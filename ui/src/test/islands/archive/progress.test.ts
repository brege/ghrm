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

  it('renders empty initially', () => {
    expect(element.querySelector('.ghrm-archive-progress')).toBeNull();
  });

  it('shows progress after startJob', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      jsonResponse({
        download_url: '/_ghrm/archive/test.zip',
        status_url: '/_ghrm/archive/status/123',
      }),
    );

    await element.startJob('/_ghrm/archive/start');
    await element.updateComplete;

    expect(element.innerHTML).toContain('ghrm-archive-progress-label');
    expect(element.innerHTML).toContain('ghrm-archive-progress-fill');
  });

  it('clears timers on disconnect', async () => {
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (url) => {
      if (requestUrl(url).includes('start')) {
        return jsonResponse({
          download_url: '/_ghrm/archive/test.zip',
          status_url: '/_ghrm/archive/status/123',
        });
      }
      return jsonResponse({
        state: 'running',
        percent: 50,
      });
    });

    await element.startJob('/_ghrm/archive/start');
    await element.updateComplete;

    const clearTimeoutSpy = vi.spyOn(globalThis, 'clearTimeout');
    element.remove();

    expect(clearTimeoutSpy).toHaveBeenCalled();
  });

  it('handles failed job gracefully', async () => {
    vi.spyOn(globalThis, 'fetch').mockRejectedValue(new Error('Network error'));

    await element.startJob('/_ghrm/archive/start');
    await element.updateComplete;
    await vi.waitFor(() => element.innerHTML.includes('failed'));

    expect(element.innerHTML).toContain('Archive failed');
  });

  it('updates progress from status poll', async () => {
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (url) => {
      if (requestUrl(url).includes('start')) {
        return jsonResponse({
          download_url: '/_ghrm/archive/test.zip',
          status_url: '/_ghrm/archive/status/123',
        });
      }
      return jsonResponse({
        state: 'complete',
        percent: 100,
        filename: 'test.zip',
      });
    });

    await element.startJob('/_ghrm/archive/start');
    await element.updateComplete;

    expect(element.innerHTML).toContain('Archive complete');
  });

  it('triggers download link creation', async () => {
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
  });

  it('hides after completion timeout', async () => {
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (url) => {
      if (requestUrl(url).includes('start')) {
        return jsonResponse({
          download_url: '/_ghrm/archive/test.zip',
          status_url: '/_ghrm/archive/status/123',
        });
      }
      return jsonResponse({
        state: 'complete',
        percent: 100,
      });
    });

    vi.useFakeTimers();
    await element.startJob('/_ghrm/archive/start');
    await element.updateComplete;
    expect(element.innerHTML).toContain('Archive complete');

    await vi.advanceTimersByTimeAsync(2000);
    await element.updateComplete;
    expect(element.querySelector('.ghrm-archive-progress')).toBeNull();
    vi.useRealTimers();
  });

  it('does not duplicate timers on repeated startJob', async () => {
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (url) => {
      if (requestUrl(url).includes('start')) {
        return jsonResponse({
          download_url: '/_ghrm/archive/test.zip',
          status_url: '/_ghrm/archive/status/123',
        });
      }
      return jsonResponse({
        state: 'running',
        percent: 50,
      });
    });

    const clearTimeoutSpy = vi.spyOn(globalThis, 'clearTimeout');

    await element.startJob('/_ghrm/archive/start');
    await element.updateComplete;

    await element.startJob('/_ghrm/archive/start2');
    await element.updateComplete;

    expect(clearTimeoutSpy).toHaveBeenCalled();
  });
});

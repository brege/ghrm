import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { copyPathFromHref, setupPathCopy } from '../path-copy';

describe('explorer path copy', () => {
  let writeText: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    });
  });

  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  it('formats explorer hrefs as copyable relative paths', () => {
    expect(copyPathFromHref('/src/main.rs')).toBe('src/main.rs');
    expect(copyPathFromHref('/assets/')).toBe('assets');
    expect(copyPathFromHref('/')).toBe('.');
    expect(copyPathFromHref('/space%20name/file.md')).toBe(
      'space name/file.md',
    );
  });

  it('adds row icon copy buttons and skips parent rows', () => {
    document.body.innerHTML = `
      <table class="ghrm-nav-table">
        <tbody>
          <tr>
            <td class="ghrm-nav-icon"><svg></svg></td>
            <td class="ghrm-nav-name"><a href="/docs/">..</a></td>
          </tr>
          <tr>
            <td class="ghrm-nav-icon"><svg></svg></td>
            <td class="ghrm-nav-name"><a href="/src/main.rs">main.rs</a></td>
          </tr>
        </tbody>
      </table>
    `;

    setupPathCopy();
    setupPathCopy();

    const buttons = document.querySelectorAll('.ghrm-nav-copy-path');
    expect(buttons).toHaveLength(1);

    const button = buttons[0] as HTMLButtonElement;
    expect(button.getAttribute('aria-label')).toBe('Copy path: src/main.rs');
    expect(button.previousElementSibling?.tagName).toBe('svg');
    expect(button.querySelector('.ghrm-nav-copy-icon')).toBeTruthy();
  });

  it('copies the row path from the icon action', async () => {
    document.body.innerHTML = `
      <table class="ghrm-nav-table">
        <tbody>
          <tr>
            <td class="ghrm-nav-icon"><svg></svg></td>
            <td class="ghrm-nav-name"><a href="/src/main.rs">main.rs</a></td>
          </tr>
        </tbody>
      </table>
    `;

    setupPathCopy();
    const button = document.querySelector<HTMLButtonElement>(
      '.ghrm-nav-copy-path',
    );
    if (!button) throw new Error('missing copy path button');
    button.click();

    await vi.waitFor(() => {
      expect(writeText).toHaveBeenCalledWith('src/main.rs');
    });
  });
});

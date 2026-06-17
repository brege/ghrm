import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { renderBlobs, renderCode } from '../adapters/code';

describe('code highlighting', () => {
  beforeEach(() => {
    document.body.innerHTML = `
      <article class="markdown-body">
        <pre><code class="language-just">build:
  cargo test</code></pre>
      </article>
    `;
  });

  afterEach(() => {
    document.body.innerHTML = '';
    delete window.hljs;
    vi.restoreAllMocks();
  });

  it('registers the just grammar once before highlighting', () => {
    const registerLanguage = vi.fn();
    const highlightElement = vi.fn();
    window.hljs = {
      registerLanguage,
      highlightElement,
    };

    renderCode();
    renderCode();

    expect(registerLanguage).toHaveBeenCalledTimes(1);
    expect(registerLanguage).toHaveBeenCalledWith('just', expect.any(Function));
    expect(highlightElement).toHaveBeenCalledTimes(1);

    const define = registerLanguage.mock.calls[0][1] as () => Record<
      string,
      unknown
    >;
    const grammar = define();
    expect(grammar.name).toBe('Just');
  });
});

describe('blob line rendering', () => {
  function makeBlob(source: string): HTMLTableSectionElement {
    document.body.innerHTML = `
      <div class="ghrm-blob">
        <div class="ghrm-blob-source"><pre><code></code></pre></div>
        <table class="ghrm-blob-table"><tbody></tbody></table>
      </div>
    `;
    const code = document.querySelector('.ghrm-blob-source code');
    if (!code) {
      throw new Error('missing blob code element');
    }
    code.textContent = source;
    return document.querySelector(
      '.ghrm-blob-table tbody',
    ) as HTMLTableSectionElement;
  }

  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('drops the terminating newline instead of rendering a phantom line', () => {
    const body = makeBlob('edition = "2024"\nstyle_edition = "2024"\n');

    renderBlobs();

    expect(body.querySelectorAll('tr')).toHaveLength(2);
    expect(body.querySelector('.ghrm-blob-eof-row')).toBeNull();
  });

  it('keeps exactly one trailing blank line', () => {
    const body = makeBlob('alpha\n\n');

    renderBlobs();

    const rows = body.querySelectorAll('tr');
    expect(rows).toHaveLength(2);
    expect(body.querySelector('.ghrm-blob-eof-row')).toBeNull();
  });

  it('marks files with no newline at end of file', () => {
    const body = makeBlob('alpha\nbeta');

    renderBlobs();

    const rows = body.querySelectorAll('tr');
    expect(rows).toHaveLength(3);
    const eof = body.querySelector('.ghrm-blob-eof-row');
    expect(eof).not.toBeNull();
    expect(eof?.textContent).toContain('No newline at end of file');
  });

  it('renders no rows for an empty file', () => {
    const body = makeBlob('');

    renderBlobs();

    expect(body.querySelectorAll('tr')).toHaveLength(0);
  });
});

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { renderCode } from '../adapters/code';

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

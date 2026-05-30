import { beforeEach, describe, expect, it } from 'vitest';
import { addCopyButtons } from '../adapters/copy';

function setUrl(path: string): void {
  window.history.pushState({}, '', path);
}

describe('copy buttons', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    setUrl('/');
  });

  it('remain available in print mode', () => {
    setUrl('/docs/readme.md?print=1');
    document.body.innerHTML = `
      <section class="ghrm-page-shell" data-ghrm-view-kind="markdown">
        <article class="markdown-body">
          <pre><code>alpha</code></pre>
        </article>
      </section>
    `;

    addCopyButtons();

    expect(document.querySelector('.ghrm-copy-button')).not.toBeNull();
  });
});

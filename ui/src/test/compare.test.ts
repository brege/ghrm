import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setupCompare } from '../compare';

const FORM_HTML = `
  <form id="ghrm-compare" class="ghrm-compare" method="get" action="/a.md" data-ghrm-compare>
    <input type="hidden" name="hidden" value="1">
    <label class="ghrm-compare-side ghrm-compare-base">
      <select name="base" aria-label="Base revision">
        <option value="HEAD" selected>HEAD</option>
        <option value="d888e48aaaa" data-timestamp="1723600000">d888e48</option>
      </select>
    </label>
    <label class="ghrm-compare-side ghrm-compare-head">
      <select name="head" aria-label="Head revision">
        <option value=":worktree" selected>Working tree</option>
      </select>
    </label>
    <button type="submit" class="ghrm-compare-apply">Diff</button>
    <button type="button" class="ghrm-compare-close" data-ghrm-compare-close aria-label="Close compare">x</button>
  </form>
`;

function containerHtml(attrs: string, compare = ''): string {
  return `
    <section class="ghrm-page-shell" data-ghrm-view-kind="source" ${attrs}>
      <div class="ghrm-explorer-header">
        <nav class="ghrm-breadcrumbs">a.md</nav>
        ${compare}
        <div class="ghrm-header-actions"></div>
      </div>
    </section>
  `;
}

function setup(
  attrs: string,
  compare = '',
): {
  container: HTMLElement;
  tools: HTMLElement;
} {
  document.body.innerHTML = containerHtml(attrs, compare);
  const container = document.querySelector('.ghrm-page-shell') as HTMLElement;
  const host = container.querySelector('.ghrm-header-actions') as HTMLElement;
  const tools = document.createElement('div');
  tools.className = 'ghrm-file-tools';
  host.prepend(tools);
  setupCompare(container, tools);
  return { container, tools };
}

function toggleButton(tools: HTMLElement): HTMLButtonElement | null {
  return tools.querySelector('[data-ghrm-compare-toggle]');
}

describe('compare controls', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('adds no button without a compare url', () => {
    const { tools } = setup('');

    expect(toggleButton(tools)).toBeNull();
  });

  it('adds a collapsed toggle before the view toggles', () => {
    const { tools } = setup('data-ghrm-compare-url="/_ghrm/compare?path=a.md"');

    const button = toggleButton(tools);
    expect(button).not.toBeNull();
    expect(button?.getAttribute('aria-expanded')).toBe('false');
    expect(button?.classList.contains('is-active')).toBe(false);
    expect(tools.firstElementChild).toBe(button);
    expect(button?.querySelector('use')?.getAttribute('href')).toContain(
      'ghrm-icon-compare',
    );
  });

  it('fetches the fragment, inserts it before the actions, and expands', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(
        async () =>
          ({ ok: true, text: async () => FORM_HTML }) as unknown as Response,
      ),
    );
    const { container, tools } = setup(
      'data-ghrm-compare-url="/_ghrm/compare?path=a.md"',
    );
    const button = toggleButton(tools) as HTMLButtonElement;

    button.click();
    await vi.waitFor(() => {
      expect(container.querySelector('[data-ghrm-compare]')).not.toBeNull();
    });

    const form = container.querySelector('[data-ghrm-compare]') as HTMLElement;
    const header = container.querySelector('.ghrm-explorer-header');
    const actions = container.querySelector('.ghrm-header-actions');
    expect(form.parentElement).toBe(header);
    expect(form.nextElementSibling).toBe(actions);
    expect(form.classList.contains('is-collapsed')).toBe(false);
    expect(form.hidden).toBe(false);
    expect(button.getAttribute('aria-expanded')).toBe('true');
    expect(document.activeElement).toBe(
      form.querySelector('select[name="base"]'),
    );
  });

  it('collapses on second activation and restores focus to the button', async () => {
    vi.stubGlobal(
      'matchMedia',
      vi.fn(() => ({ matches: true }) as MediaQueryList),
    );
    const { container, tools } = setup(
      'data-ghrm-compare-url="/_ghrm/compare?path=a.md"',
      FORM_HTML,
    );
    const button = toggleButton(tools) as HTMLButtonElement;
    const form = container.querySelector('[data-ghrm-compare]') as HTMLElement;
    expect(button.getAttribute('aria-expanded')).toBe('true');

    button.click();

    expect(form.classList.contains('is-collapsed')).toBe(true);
    expect(form.hidden).toBe(true);
    expect(button.getAttribute('aria-expanded')).toBe('false');
    expect(document.activeElement).toBe(button);

    button.click();

    expect(form.hidden).toBe(false);
    expect(form.classList.contains('is-collapsed')).toBe(false);
    expect(button.getAttribute('aria-expanded')).toBe('true');
  });

  it('starts expanded for a server-rendered bar and closes from its control', () => {
    vi.stubGlobal(
      'matchMedia',
      vi.fn(() => ({ matches: true }) as MediaQueryList),
    );
    const { container, tools } = setup(
      'data-ghrm-compare-url="/_ghrm/compare?path=a.md" data-ghrm-diff="HEAD..:worktree"',
      FORM_HTML,
    );
    const button = toggleButton(tools) as HTMLButtonElement;
    const form = container.querySelector('[data-ghrm-compare]') as HTMLElement;
    expect(button.getAttribute('aria-expanded')).toBe('true');
    expect(button.classList.contains('is-active')).toBe(true);

    (
      form.querySelector('[data-ghrm-compare-close]') as HTMLButtonElement
    ).click();

    expect(form.hidden).toBe(true);
    expect(button.getAttribute('aria-expanded')).toBe('false');
  });

  it('submits the form when a select changes', () => {
    const { container } = setup(
      'data-ghrm-compare-url="/_ghrm/compare?path=a.md"',
      FORM_HTML,
    );
    const form = container.querySelector(
      '[data-ghrm-compare]',
    ) as HTMLFormElement;
    let submitted = false;
    form.addEventListener('submit', (event) => {
      event.preventDefault();
      submitted = true;
    });

    const base = form.querySelector('select[name="base"]') as HTMLSelectElement;
    base.value = 'd888e48aaaa';
    base.dispatchEvent(new Event('change', { bubbles: true }));

    expect(submitted).toBe(true);
  });

  it('titles commit options with relative ages', () => {
    const { container } = setup(
      'data-ghrm-compare-url="/_ghrm/compare?path=a.md"',
      FORM_HTML,
    );

    const option = container.querySelector(
      'option[data-timestamp]',
    ) as HTMLOptionElement;
    expect(option.title).toContain('ago');
  });
});

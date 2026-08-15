import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setupCompare } from '../compare';

const FORM_HTML = `
  <form id="ghrm-compare" class="ghrm-compare" method="get" action="/a.md" data-ghrm-compare>
    <div class="ghrm-compare-inner">
      <input type="hidden" name="hidden" value="1">
      <label class="ghrm-compare-side ghrm-compare-base">
        <select name="base" aria-label="Base revision">
          <option value="HEAD" data-timestamp="1723600000" selected>HEAD</option>
          <option value="d888e48aaaa" data-timestamp="1723500000">d888e48</option>
        </select>
        <time data-ghrm-compare-time></time>
      </label>
      <span class="ghrm-compare-dots">..</span>
      <label class="ghrm-compare-side ghrm-compare-head">
        <select name="head" aria-label="Head revision">
          <option value=":worktree" data-time-label="now" selected>Working tree</option>
          <option value=":index" data-time-label="staged">Staged</option>
        </select>
        <time data-ghrm-compare-time></time>
      </label>
    </div>
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
    vi.useRealTimers();
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

  it('leaves an active diff through the compare toggle without ref fields', () => {
    vi.stubGlobal(
      'matchMedia',
      vi.fn(() => ({ matches: true }) as MediaQueryList),
    );
    const { container, tools } = setup(
      'data-ghrm-compare-url="/_ghrm/compare?path=a.md" data-ghrm-diff="HEAD..:worktree"',
      FORM_HTML,
    );
    const button = toggleButton(tools) as HTMLButtonElement;
    const form = container.querySelector(
      '[data-ghrm-compare]',
    ) as HTMLFormElement;
    let submitted: FormData | null = null;
    form.addEventListener('submit', (event) => {
      event.preventDefault();
      submitted = new FormData(form);
    });
    expect(button.getAttribute('aria-expanded')).toBe('true');
    expect(button.classList.contains('is-active')).toBe(true);

    button.click();

    expect(form.hidden).toBe(true);
    expect(button.getAttribute('aria-expanded')).toBe('false');
    expect(submitted?.get('hidden')).toBe('1');
    expect(submitted?.has('base')).toBe(false);
    expect(submitted?.has('head')).toBe(false);
  });

  it('debounces automatic submission while a select is changing', async () => {
    vi.useFakeTimers();
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

    await vi.advanceTimersByTimeAsync(400);
    expect(submitted).toBe(false);

    const head = form.querySelector('select[name="head"]') as HTMLSelectElement;
    head.value = ':index';
    head.dispatchEvent(new Event('change', { bubbles: true }));
    await vi.advanceTimersByTimeAsync(599);
    expect(submitted).toBe(false);
    await vi.advanceTimersByTimeAsync(1);
    expect(submitted).toBe(true);
  });

  it('shows the selected ref time and mutable state label', () => {
    const { container } = setup(
      'data-ghrm-compare-url="/_ghrm/compare?path=a.md"',
      FORM_HTML,
    );

    const option = container.querySelector(
      'option[data-timestamp]',
    ) as HTMLOptionElement;
    expect(option.title).toContain('ago');
    const times = container.querySelectorAll('[data-ghrm-compare-time]');
    expect(times[0]?.textContent).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/);
    expect(times[1]?.textContent).toBe('now');
  });

  it('restores the previous refs when the compare row is loaded again', async () => {
    const first = setup(
      'data-ghrm-compare-url="/_ghrm/compare?path=a.md"',
      FORM_HTML,
    );
    const firstForm = first.container.querySelector(
      '[data-ghrm-compare]',
    ) as HTMLFormElement;
    const base = firstForm.querySelector(
      'select[name="base"]',
    ) as HTMLSelectElement;
    const head = firstForm.querySelector(
      'select[name="head"]',
    ) as HTMLSelectElement;
    base.value = 'd888e48aaaa';
    base.dispatchEvent(new Event('change', { bubbles: true }));
    head.value = ':index';
    head.dispatchEvent(new Event('change', { bubbles: true }));
    (toggleButton(first.tools) as HTMLButtonElement).click();

    vi.stubGlobal(
      'fetch',
      vi.fn(
        async () =>
          ({ ok: true, text: async () => FORM_HTML }) as unknown as Response,
      ),
    );
    const second = setup('data-ghrm-compare-url="/_ghrm/compare?path=a.md"');
    (toggleButton(second.tools) as HTMLButtonElement).click();
    await vi.waitFor(() => {
      expect(
        second.container.querySelector('[data-ghrm-compare]'),
      ).not.toBeNull();
    });

    const secondForm = second.container.querySelector(
      '[data-ghrm-compare]',
    ) as HTMLFormElement;
    expect(
      (secondForm.querySelector('select[name="base"]') as HTMLSelectElement)
        .value,
    ).toBe('d888e48aaaa');
    expect(
      (secondForm.querySelector('select[name="head"]') as HTMLSelectElement)
        .value,
    ).toBe(':index');
  });
});

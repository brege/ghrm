import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setupCompare } from '../../file/compare';

const FORM_HTML = `
  <form id="ghrm-compare" class="ghrm-compare" method="get" action="/a.md" data-ghrm-compare>
    <div class="ghrm-compare-inner">
      <input type="hidden" name="hidden" value="1">
      <input type="hidden" name="base" value="HEAD" data-ghrm-compare-input="base">
      <input type="hidden" name="head" value=":worktree" data-ghrm-compare-input="head">
      <div class="ghrm-compare-controls">
        <div class="ghrm-compare-side" data-ghrm-compare-side="base">
          <button type="button" data-ghrm-menu-toggle aria-controls="base-menu">
            <span data-ghrm-compare-picker-label>HEAD</span>
          </button>
          <time data-ghrm-compare-time></time>
          <div id="base-menu" data-ghrm-menu-panel hidden>
            <button type="button" data-ghrm-compare-option="HEAD" data-timestamp="1723600000" aria-checked="true">
              <span data-ghrm-compare-option-label>HEAD</span>
              <time data-ghrm-compare-option-time></time>
            </button>
            <button type="button" data-ghrm-compare-option="d888e48aaaa" data-timestamp="1723500000" aria-checked="false">
              <span data-ghrm-compare-option-label>
                <span>d888e48</span>
                <span data-ghrm-compare-message><span>change subject</span></span>
              </span>
              <time data-ghrm-compare-option-time></time>
            </button>
          </div>
        </div>
        <span class="ghrm-compare-dots">..</span>
        <div class="ghrm-compare-side" data-ghrm-compare-side="head">
          <button type="button" data-ghrm-menu-toggle aria-controls="head-menu">
            <span data-ghrm-compare-picker-label>Working tree</span>
          </button>
          <time data-ghrm-compare-time></time>
          <div id="head-menu" data-ghrm-menu-panel hidden>
            <button type="button" data-ghrm-compare-option=":worktree" data-time-label="now" aria-checked="true">
              <span data-ghrm-compare-option-label>Working tree</span>
              <time data-ghrm-compare-option-time>now</time>
            </button>
            <button type="button" data-ghrm-compare-option=":index" data-time-label="staged" aria-checked="false">
              <span data-ghrm-compare-option-label>Staged</span>
              <time data-ghrm-compare-option-time>staged</time>
            </button>
          </div>
        </div>
      </div>
      <div data-ghrm-compare-progress hidden>
        <span data-ghrm-compare-progress-label>Loading diff</span>
        <div class="ghrm-progress-track"><div class="ghrm-progress-fill is-periodic"></div></div>
      </div>
    </div>
  </form>
`;

interface HtmxContext {
  source?: Element;
  target?: Element | string;
  swap?: string;
}

function stubHtmx(html = FORM_HTML) {
  const ajax = vi.fn(
    async (_verb: string, _url: string, context: HtmxContext) => {
      if (!(context.target instanceof Element)) {
        throw new Error('missing htmx target');
      }
      if (context.swap !== 'beforebegin') {
        throw new Error('unexpected htmx swap');
      }
      context.target.insertAdjacentHTML('beforebegin', html);
    },
  );
  vi.stubGlobal('htmx', { ajax });
  return ajax;
}

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
): { container: HTMLElement; tools: HTMLElement; toggles: HTMLElement } {
  document.body.innerHTML = containerHtml(attrs, compare);
  const container = document.querySelector('.ghrm-page-shell') as HTMLElement;
  const host = container.querySelector('.ghrm-header-actions') as HTMLElement;
  const tools = document.createElement('div');
  tools.className = 'ghrm-file-tools';
  host.prepend(tools);
  const toggles = document.createElement('div');
  toggles.className = 'ghrm-file-toggles';
  tools.append(toggles);
  setupCompare(container, toggles);
  return { container, tools, toggles };
}

function toggleButton(tools: HTMLElement): HTMLButtonElement | null {
  return tools.querySelector('[data-ghrm-compare-toggle]');
}

function option(
  form: HTMLFormElement,
  side: 'base' | 'head',
  value: string,
): HTMLButtonElement {
  const found = [
    ...form.querySelectorAll<HTMLButtonElement>(
      `[data-ghrm-compare-side="${side}"] [data-ghrm-compare-option]`,
    ),
  ].find((item) => item.dataset.ghrmCompareOption === value);
  if (!found) throw new Error(`missing ${side} option ${value}`);
  return found;
}

describe('compare controls', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('adds no button without a compare url', () => {
    const { tools } = setup('');

    expect(toggleButton(tools)).toBeNull();
  });

  it('adds a collapsed toggle before the view toggles', () => {
    const { tools, toggles } = setup(
      'data-ghrm-compare-url="/_ghrm/compare?path=a.md"',
    );

    const button = toggleButton(tools);
    expect(button?.getAttribute('aria-expanded')).toBe('false');
    expect(button?.classList.contains('is-active')).toBe(false);
    expect(toggles.firstElementChild).toBe(button);
    expect(button?.querySelector('use')?.getAttribute('href')).toContain(
      'ghrm-icon-compare',
    );
  });

  it('loads the fragment through htmx before the actions and expands', async () => {
    const ajax = stubHtmx();
    const { container, tools } = setup(
      'data-ghrm-compare-url="/_ghrm/compare?path=a.md"',
    );
    const button = toggleButton(tools) as HTMLButtonElement;
    const actions = container.querySelector(
      '.ghrm-header-actions',
    ) as HTMLElement;

    button.click();
    await vi.waitFor(() => {
      expect(container.querySelector('[data-ghrm-compare]')).not.toBeNull();
    });

    const form = container.querySelector('[data-ghrm-compare]') as HTMLElement;
    expect(form.parentElement).toBe(
      container.querySelector('.ghrm-explorer-header'),
    );
    expect(form.nextElementSibling).toBe(
      container.querySelector('.ghrm-header-actions'),
    );
    expect(form.classList.contains('is-collapsed')).toBe(false);
    expect(button.getAttribute('aria-expanded')).toBe('true');
    expect(button.getAttribute('hx-push-url')).toBe('false');
    expect(ajax).toHaveBeenCalledWith('GET', '/_ghrm/compare?path=a.md', {
      source: button,
      target: actions,
      swap: 'beforebegin',
    });
    expect(document.activeElement).toBe(
      form.querySelector('[data-ghrm-menu-toggle]'),
    );
  });

  it('collapses on second activation and restores focus to the button', () => {
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

  it('leaves a diff without ref fields and shows file load progress', () => {
    vi.stubGlobal(
      'matchMedia',
      vi.fn(() => ({ matches: true }) as MediaQueryList),
    );
    const { container, tools } = setup(
      'data-ghrm-compare-url="/_ghrm/compare?path=a.md" data-ghrm-diff="HEAD..:worktree"',
      FORM_HTML,
    );
    const form = container.querySelector(
      '[data-ghrm-compare]',
    ) as HTMLFormElement;
    let submitted: FormData | null = null;
    form.addEventListener('submit', (event) => {
      event.preventDefault();
      submitted = new FormData(form);
    });

    (toggleButton(tools) as HTMLButtonElement).click();

    expect(submitted?.get('hidden')).toBe('1');
    expect(submitted?.has('base')).toBe(false);
    expect(submitted?.has('head')).toBe(false);
    expect(
      form.querySelector<HTMLElement>('[data-ghrm-compare-progress]')?.hidden,
    ).toBe(false);
    expect(
      form.querySelector('[data-ghrm-compare-progress-label]')?.textContent,
    ).toBe('Loading file');
  });

  it('updates hidden refs and debounces automatic submission', async () => {
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

    option(form, 'base', 'd888e48aaaa').click();
    expect(
      form.querySelector<HTMLInputElement>('[data-ghrm-compare-input="base"]')
        ?.value,
    ).toBe('d888e48aaaa');
    await vi.advanceTimersByTimeAsync(400);
    expect(submitted).toBe(false);

    option(form, 'head', ':index').click();
    expect(
      form.querySelector<HTMLInputElement>('[data-ghrm-compare-input="head"]')
        ?.value,
    ).toBe(':index');
    await vi.advanceTimersByTimeAsync(599);
    expect(submitted).toBe(false);
    await vi.advanceTimersByTimeAsync(1);
    expect(submitted).toBe(true);
    expect(
      form.querySelector<HTMLElement>('[data-ghrm-compare-progress]')?.hidden,
    ).toBe(false);
    expect(
      form.querySelector('[data-ghrm-compare-progress-label]')?.textContent,
    ).toBe('Loading diff');
  });

  it('shows dates at the right-side option coordinate and selected time', () => {
    const { container } = setup(
      'data-ghrm-compare-url="/_ghrm/compare?path=a.md"',
      FORM_HTML,
    );

    const optionTime = container.querySelector(
      '[data-timestamp] [data-ghrm-compare-option-time]',
    ) as HTMLTimeElement;
    expect(optionTime.textContent).toContain('ago');
    expect(optionTime.title).not.toBe('');
    const times = container.querySelectorAll('[data-ghrm-compare-time]');
    expect(times[0]?.textContent).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/);
    expect(times[1]?.textContent).toBe('now');
  });

  it('scrolls only an overflowing commit message on hover and focus', () => {
    const { container } = setup(
      'data-ghrm-compare-url="/_ghrm/compare?path=a.md"',
      FORM_HTML,
    );
    const form = container.querySelector(
      '[data-ghrm-compare]',
    ) as HTMLFormElement;
    const commit = option(form, 'base', 'd888e48aaaa');
    const message = commit.querySelector(
      '[data-ghrm-compare-message]',
    ) as HTMLElement;
    Object.defineProperties(message, {
      clientWidth: { configurable: true, value: 100 },
      scrollWidth: { configurable: true, value: 244 },
    });

    commit.dispatchEvent(new MouseEvent('mouseenter'));

    expect(message.classList.contains('is-scrolling')).toBe(true);
    expect(
      message.style.getPropertyValue('--ghrm-compare-scroll-distance'),
    ).toBe('-144px');
    expect(
      message.style.getPropertyValue('--ghrm-compare-scroll-duration'),
    ).toBe('5.40s');

    commit.dispatchEvent(new MouseEvent('mouseleave'));
    expect(message.classList.contains('is-scrolling')).toBe(false);

    commit.focus();
    expect(message.classList.contains('is-scrolling')).toBe(true);
    commit.blur();
    expect(message.classList.contains('is-scrolling')).toBe(false);

    Object.defineProperty(message, 'scrollWidth', {
      configurable: true,
      value: 100,
    });
    commit.dispatchEvent(new MouseEvent('mouseenter'));
    expect(message.classList.contains('is-scrolling')).toBe(false);
  });

  it('clears progress when the compare request finishes without replacement', () => {
    const { container } = setup(
      'data-ghrm-compare-url="/_ghrm/compare?path=a.md"',
      FORM_HTML,
    );
    const form = container.querySelector(
      '[data-ghrm-compare]',
    ) as HTMLFormElement;
    form.addEventListener('submit', (event) => event.preventDefault());

    form.requestSubmit();
    form.dispatchEvent(new Event('htmx:afterRequest'));

    expect(
      form.querySelector<HTMLElement>('[data-ghrm-compare-progress]')?.hidden,
    ).toBe(true);
    expect(
      form.querySelector<HTMLElement>('.ghrm-compare-controls')?.hidden,
    ).toBe(false);
  });

  it('restores the previous refs and re-submits the diff when reloaded', async () => {
    const first = setup(
      'data-ghrm-compare-url="/_ghrm/compare?path=a.md"',
      FORM_HTML,
    );
    const firstForm = first.container.querySelector(
      '[data-ghrm-compare]',
    ) as HTMLFormElement;
    option(firstForm, 'base', 'd888e48aaaa').click();
    option(firstForm, 'head', ':index').click();
    (toggleButton(first.tools) as HTMLButtonElement).click();

    stubHtmx();
    let resubmitted = false;
    const guard = (event: Event) => {
      event.preventDefault();
      resubmitted = true;
    };
    document.addEventListener('submit', guard);
    const second = setup('data-ghrm-compare-url="/_ghrm/compare?path=a.md"');
    (toggleButton(second.tools) as HTMLButtonElement).click();
    await vi.waitFor(() => {
      expect(
        second.container.querySelector('[data-ghrm-compare]'),
      ).not.toBeNull();
    });
    document.removeEventListener('submit', guard);

    const secondForm = second.container.querySelector(
      '[data-ghrm-compare]',
    ) as HTMLFormElement;
    expect(
      secondForm.querySelector<HTMLInputElement>(
        '[data-ghrm-compare-input="base"]',
      )?.value,
    ).toBe('d888e48aaaa');
    expect(
      secondForm.querySelector<HTMLInputElement>(
        '[data-ghrm-compare-input="head"]',
      )?.value,
    ).toBe(':index');
    expect(
      secondForm.querySelector(
        '[data-ghrm-compare-side="base"] [data-ghrm-compare-picker-label]',
      )?.textContent,
    ).toBe('d888e48 change subject');
    expect(resubmitted).toBe(true);
  });
});

import { formatRelative, icon, qselFrom } from './dom';

const COLLAPSE_MS = 320;

const collapseTimers = new WeakMap<
  HTMLFormElement,
  ReturnType<typeof setTimeout>
>();

function formOf(container: HTMLElement): HTMLFormElement | null {
  const form = container.querySelector('[data-ghrm-compare]');
  return form instanceof HTMLFormElement ? form : null;
}

function reducedMotion(): boolean {
  return (
    window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false
  );
}

function decorateAges(form: HTMLFormElement): void {
  for (const option of form.querySelectorAll('option[data-timestamp]')) {
    if (!(option instanceof HTMLOptionElement)) continue;
    const ts = Number(option.dataset.timestamp);
    if (Number.isFinite(ts) && ts > 0) {
      option.title = formatRelative(ts);
    }
  }
}

function setExpanded(button: HTMLButtonElement, expanded: boolean): void {
  button.setAttribute('aria-expanded', expanded ? 'true' : 'false');
  button.classList.toggle('is-active', expanded);
}

function expand(form: HTMLFormElement, button: HTMLButtonElement): void {
  const timer = collapseTimers.get(form);
  if (timer !== undefined) {
    clearTimeout(timer);
    collapseTimers.delete(form);
  }
  form.hidden = false;
  // A reflow between unhiding and dropping the class lets the width
  // transition run instead of snapping open.
  void form.offsetWidth;
  form.classList.remove('is-collapsed');
  setExpanded(button, true);
  qselFrom(form, 'select[name="base"]')?.focus();
}

function collapse(form: HTMLFormElement, button: HTMLButtonElement): void {
  form.classList.add('is-collapsed');
  setExpanded(button, false);
  if (reducedMotion()) {
    form.hidden = true;
  } else {
    collapseTimers.set(
      form,
      setTimeout(() => {
        form.hidden = true;
        collapseTimers.delete(form);
      }, COLLAPSE_MS),
    );
  }
  button.focus();
}

function wireForm(form: HTMLFormElement, button: HTMLButtonElement): void {
  if (form.dataset.ghrmCompareWired === '1') return;
  form.dataset.ghrmCompareWired = '1';
  decorateAges(form);

  form.addEventListener('click', (event) => {
    const target = event.target instanceof Element ? event.target : null;
    if (target?.closest('[data-ghrm-compare-close]')) {
      collapse(form, button);
    }
  });

  form.addEventListener('change', (event) => {
    if (!(event.target instanceof HTMLSelectElement)) return;
    if (typeof form.requestSubmit === 'function') {
      form.requestSubmit();
    } else {
      form.submit();
    }
  });
}

async function loadForm(
  container: HTMLElement,
  button: HTMLButtonElement,
  url: string,
): Promise<void> {
  button.disabled = true;
  try {
    const response = await fetch(url, {
      headers: { Accept: 'text/html' },
    }).catch(() => null);
    if (!response || !response.ok) return;
    const template = document.createElement('template');
    template.innerHTML = (await response.text()).trim();
    const form = template.content.querySelector('[data-ghrm-compare]');
    const header = qselFrom(container, '.ghrm-explorer-header');
    const actions = header ? qselFrom(header, '.ghrm-header-actions') : null;
    if (!(form instanceof HTMLFormElement) || !header || !actions) return;
    form.classList.add('is-collapsed');
    header.insertBefore(form, actions);
    wireForm(form, button);
    expand(form, button);
  } finally {
    button.disabled = false;
  }
}

export function setupCompare(container: HTMLElement, tools: HTMLElement): void {
  const url = container.dataset.ghrmCompareUrl;
  if (!url) return;

  const button = document.createElement('button');
  button.type = 'button';
  button.className = 'ghrm-file-toggle ghrm-compare-toggle';
  button.dataset.ghrmCompareToggle = '1';
  button.innerHTML = icon('compare');
  button.setAttribute('aria-controls', 'ghrm-compare');
  button.setAttribute('aria-label', 'Compare revisions');
  button.title = 'Compare revisions';

  const initial = formOf(container);
  setExpanded(button, initial !== null && !initial.hidden);
  if (initial) {
    wireForm(initial, button);
  }

  button.addEventListener('click', () => {
    const form = formOf(container);
    if (!form) {
      void loadForm(container, button, url);
      return;
    }
    wireForm(form, button);
    if (form.hidden || form.classList.contains('is-collapsed')) {
      expand(form, button);
    } else {
      collapse(form, button);
    }
  });

  tools.prepend(button);
}

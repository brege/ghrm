import { formatAbsolute, formatRelative, icon, qselFrom } from './dom';

const COLLAPSE_MS = 280;
const SUBMIT_MS = 600;

interface Selection {
  key: string;
  base: string;
  head: string;
}

interface Inputs {
  base: HTMLInputElement;
  head: HTMLInputElement;
}

const collapseTimers = new WeakMap<
  HTMLFormElement,
  ReturnType<typeof setTimeout>
>();
const submitTimers = new WeakMap<
  HTMLFormElement,
  ReturnType<typeof setTimeout>
>();
let selection: Selection | null = null;

function formOf(container: HTMLElement): HTMLFormElement | null {
  const form = container.querySelector('[data-ghrm-compare]');
  return form instanceof HTMLFormElement ? form : null;
}

function reducedMotion(): boolean {
  return (
    window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false
  );
}

function inputsOf(form: HTMLFormElement): Inputs | null {
  const base = form.querySelector('[data-ghrm-compare-input="base"]');
  const head = form.querySelector('[data-ghrm-compare-input="head"]');
  return base instanceof HTMLInputElement && head instanceof HTMLInputElement
    ? { base, head }
    : null;
}

function selectionKey(form: HTMLFormElement): string {
  return form.getAttribute('action') ?? form.action;
}

function rememberSelection(form: HTMLFormElement): void {
  const inputs = inputsOf(form);
  if (!inputs) return;
  selection = {
    key: selectionKey(form),
    base: inputs.base.value,
    head: inputs.head.value,
  };
}

function sideOptions(side: Element): HTMLElement[] {
  return [...side.querySelectorAll('[data-ghrm-compare-option]')].filter(
    (option): option is HTMLElement => option instanceof HTMLElement,
  );
}

function optionLabel(option: HTMLElement): string {
  const label = option.querySelector('[data-ghrm-compare-option-label]');
  return (label?.textContent ?? '').replace(/\s+/g, ' ').trim();
}

function setMessageScroll(option: HTMLElement, active: boolean): void {
  const message = option.querySelector('[data-ghrm-compare-message]');
  if (!(message instanceof HTMLElement)) return;
  message.classList.remove('is-scrolling');
  message.style.removeProperty('--ghrm-compare-scroll-distance');
  message.style.removeProperty('--ghrm-compare-scroll-duration');
  if (!active) return;

  const distance = message.scrollWidth - message.clientWidth;
  if (distance <= 0) return;
  const duration = Math.max(2.4, distance / 36 + 1.4);
  message.style.setProperty('--ghrm-compare-scroll-distance', `${-distance}px`);
  message.style.setProperty(
    '--ghrm-compare-scroll-duration',
    `${duration.toFixed(2)}s`,
  );
  // Restart the measured animation when pointer or keyboard focus returns.
  void message.offsetWidth;
  message.classList.add('is-scrolling');
}

function wireMessageScroll(form: HTMLFormElement): void {
  for (const option of form.querySelectorAll('[data-ghrm-compare-option]')) {
    if (!(option instanceof HTMLElement)) continue;
    if (!option.querySelector('[data-ghrm-compare-message]')) continue;
    option.addEventListener('mouseenter', () => setMessageScroll(option, true));
    option.addEventListener('mouseleave', () =>
      setMessageScroll(option, false),
    );
    option.addEventListener('focus', () => setMessageScroll(option, true));
    option.addEventListener('blur', () => setMessageScroll(option, false));
  }
}

function inputForSide(
  form: HTMLFormElement,
  side: HTMLElement,
): HTMLInputElement | null {
  const kind = side.dataset.ghrmCompareSide;
  const input = kind
    ? form.querySelector(`[data-ghrm-compare-input="${kind}"]`)
    : null;
  return input instanceof HTMLInputElement ? input : null;
}

function localDateTime(date: Date): string {
  const pad = (value: number) => value.toString().padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function setSelectedTime(side: HTMLElement, option: HTMLElement): void {
  const output = side.querySelector('[data-ghrm-compare-time]');
  if (!(output instanceof HTMLTimeElement)) return;
  const timestamp = Number(option.dataset.timestamp);
  const date = new Date(timestamp * 1000);
  if (
    Number.isFinite(timestamp) &&
    timestamp > 0 &&
    Number.isFinite(date.getTime())
  ) {
    output.textContent = localDateTime(date);
    output.dateTime = date.toISOString();
    output.title = formatAbsolute(timestamp);
  } else {
    output.textContent = option.dataset.timeLabel ?? '';
    output.removeAttribute('datetime');
    output.removeAttribute('title');
  }
}

function selectOption(
  form: HTMLFormElement,
  side: HTMLElement,
  option: HTMLElement,
): void {
  const input = inputForSide(form, side);
  const value = option.dataset.ghrmCompareOption;
  if (!input || value === undefined) return;
  input.value = value;
  for (const current of sideOptions(side)) {
    const active = current === option;
    current.classList.toggle('is-active', active);
    current.setAttribute('aria-checked', active ? 'true' : 'false');
  }
  const label = side.querySelector('[data-ghrm-compare-picker-label]');
  if (label) label.textContent = optionLabel(option);
  setSelectedTime(side, option);
}

function restoreSelection(form: HTMLFormElement): boolean {
  const saved = selection;
  const inputs = inputsOf(form);
  if (!saved || saved.key !== selectionKey(form) || !inputs) return false;
  let restored = false;
  for (const side of form.querySelectorAll('[data-ghrm-compare-side]')) {
    if (!(side instanceof HTMLElement)) continue;
    const input = inputForSide(form, side);
    if (!input) continue;
    const value = input === inputs.base ? saved.base : saved.head;
    const option = sideOptions(side).find(
      (current) => current.dataset.ghrmCompareOption === value,
    );
    if (option) {
      selectOption(form, side, option);
      restored = true;
    }
  }
  return restored;
}

function decorateTimes(form: HTMLFormElement): void {
  for (const option of form.querySelectorAll(
    '[data-ghrm-compare-option][data-timestamp]',
  )) {
    if (!(option instanceof HTMLElement)) continue;
    const timestamp = Number(option.dataset.timestamp);
    const time = option.querySelector('[data-ghrm-compare-option-time]');
    if (
      !(time instanceof HTMLTimeElement) ||
      !Number.isFinite(timestamp) ||
      timestamp <= 0
    ) {
      continue;
    }
    const date = new Date(timestamp * 1000);
    time.textContent = formatRelative(timestamp);
    time.dateTime = date.toISOString();
    time.title = formatAbsolute(timestamp);
  }

  for (const side of form.querySelectorAll('[data-ghrm-compare-side]')) {
    if (!(side instanceof HTMLElement)) continue;
    const input = inputForSide(form, side);
    const option = sideOptions(side).find(
      (current) => current.dataset.ghrmCompareOption === input?.value,
    );
    if (option) setSelectedTime(side, option);
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
  // Force layout before removing the collapsed class so the row animates.
  void form.offsetWidth;
  form.classList.remove('is-collapsed');
  setExpanded(button, true);
  decorateTimes(form);
  qselFrom(form, '[data-ghrm-menu-toggle]')?.focus();
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

function cancelSubmit(form: HTMLFormElement): void {
  const timer = submitTimers.get(form);
  if (timer === undefined) return;
  clearTimeout(timer);
  submitTimers.delete(form);
}

function scheduleSubmit(form: HTMLFormElement): void {
  cancelSubmit(form);
  submitTimers.set(
    form,
    setTimeout(() => {
      submitTimers.delete(form);
      form.requestSubmit();
    }, SUBMIT_MS),
  );
}

function setLoading(form: HTMLFormElement, loading: boolean): void {
  const controls = form.querySelector('.ghrm-compare-controls');
  const progress = form.querySelector('[data-ghrm-compare-progress]');
  if (
    !(controls instanceof HTMLElement) ||
    !(progress instanceof HTMLElement)
  ) {
    return;
  }
  controls.hidden = loading;
  progress.hidden = !loading;
  form.classList.toggle('is-loading', loading);
  if (loading) {
    const label = progress.querySelector('[data-ghrm-compare-progress-label]');
    if (label) {
      label.textContent =
        form.dataset.ghrmCompareExit === '1' ? 'Loading file' : 'Loading diff';
    }
  } else {
    delete form.dataset.ghrmCompareExit;
  }
}

function leaveDiff(form: HTMLFormElement): void {
  for (const input of form.querySelectorAll('[data-ghrm-compare-input]')) {
    input.removeAttribute('name');
  }
  form.dataset.ghrmCompareExit = '1';
  form.requestSubmit();
}

function wireForm(form: HTMLFormElement): void {
  if (form.dataset.ghrmCompareWired === '1') return;
  form.dataset.ghrmCompareWired = '1';
  decorateTimes(form);
  wireMessageScroll(form);

  form.addEventListener('click', (event) => {
    const target = event.target instanceof Element ? event.target : null;
    const option = target?.closest('[data-ghrm-compare-option]');
    const side = option?.closest('[data-ghrm-compare-side]');
    if (!(option instanceof HTMLElement) || !(side instanceof HTMLElement)) {
      return;
    }
    selectOption(form, side, option);
    rememberSelection(form);
    scheduleSubmit(form);
  });

  form.addEventListener('submit', () => setLoading(form, true));
  form.addEventListener('htmx:afterRequest', () => setLoading(form, false));
}

async function loadForm(
  container: HTMLElement,
  button: HTMLButtonElement,
  url: string,
): Promise<void> {
  const htmx = window.htmx;
  if (!htmx) throw new Error('htmx is unavailable');
  const actions = qselFrom(container, '.ghrm-header-actions');
  if (!actions) throw new Error('missing file header actions');

  button.disabled = true;
  try {
    await htmx.ajax('GET', url, {
      source: button,
      target: actions,
      swap: 'beforebegin',
    });
    const form = formOf(container);
    if (!form) throw new Error('compare response is missing its form');
    wireForm(form);
    if (restoreSelection(form)) {
      // Re-render the diff the compare bar was showing before it was closed.
      form.requestSubmit();
      return;
    }
    form.classList.add('is-collapsed');
    expand(form, button);
  } finally {
    button.disabled = false;
  }
}

export function setupCompare(container: HTMLElement, group: HTMLElement): void {
  const url = container.dataset.ghrmCompareUrl;
  if (!url) return;

  const button = document.createElement('button');
  button.type = 'button';
  button.className = 'ghrm-file-toggle ghrm-compare-toggle';
  button.dataset.ghrmCompareToggle = '1';
  button.innerHTML = icon('compare');
  button.setAttribute('aria-controls', 'ghrm-compare');
  button.setAttribute('aria-label', 'Compare revisions');
  button.setAttribute('hx-push-url', 'false');
  button.title = 'Compare revisions';

  const initial = formOf(container);
  setExpanded(button, initial !== null && !initial.hidden);
  if (initial) {
    rememberSelection(initial);
    wireForm(initial);
  }

  button.addEventListener('click', () => {
    const form = formOf(container);
    if (!form) {
      void loadForm(container, button, url);
      return;
    }
    wireForm(form);
    if (form.hidden || form.classList.contains('is-collapsed')) {
      expand(form, button);
    } else {
      rememberSelection(form);
      cancelSubmit(form);
      collapse(form, button);
      if (container.dataset.ghrmDiff) {
        leaveDiff(form);
      }
    }
  });

  group.prepend(button);
}

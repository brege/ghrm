import { formatRelative, icon, qselFrom } from './dom';

const COLLAPSE_MS = 280;
const SUBMIT_MS = 600;

interface Selection {
  key: string;
  base: string;
  head: string;
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

function selectsOf(
  form: HTMLFormElement,
): { base: HTMLSelectElement; head: HTMLSelectElement } | null {
  const base = form.querySelector('select[name="base"]');
  const head = form.querySelector('select[name="head"]');
  return base instanceof HTMLSelectElement && head instanceof HTMLSelectElement
    ? { base, head }
    : null;
}

function selectionKey(form: HTMLFormElement): string {
  return form.getAttribute('action') ?? form.action;
}

function rememberSelection(form: HTMLFormElement): void {
  const selects = selectsOf(form);
  if (!selects) return;
  selection = {
    key: selectionKey(form),
    base: selects.base.value,
    head: selects.head.value,
  };
}

function restoreSelection(form: HTMLFormElement): void {
  const saved = selection;
  const selects = selectsOf(form);
  if (!saved || saved.key !== selectionKey(form) || !selects) return;
  if ([...selects.base.options].some((option) => option.value === saved.base)) {
    selects.base.value = saved.base;
  }
  if ([...selects.head.options].some((option) => option.value === saved.head)) {
    selects.head.value = saved.head;
  }
}

function localDateTime(date: Date): string {
  const pad = (value: number) => value.toString().padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function decorateTimes(form: HTMLFormElement): void {
  for (const option of form.querySelectorAll('option[data-timestamp]')) {
    if (!(option instanceof HTMLOptionElement)) continue;
    const ts = Number(option.dataset.timestamp);
    if (Number.isFinite(ts) && ts > 0) {
      option.title = formatRelative(ts);
    }
  }

  for (const side of form.querySelectorAll('.ghrm-compare-side')) {
    const select = side.querySelector('select');
    const output = side.querySelector('[data-ghrm-compare-time]');
    if (
      !(select instanceof HTMLSelectElement) ||
      !(output instanceof HTMLElement)
    ) {
      continue;
    }
    const option = select.selectedOptions.item(0);
    const timestamp = Number(option?.dataset.timestamp);
    const date = new Date(timestamp * 1000);
    if (
      Number.isFinite(timestamp) &&
      timestamp > 0 &&
      Number.isFinite(date.getTime())
    ) {
      output.textContent = localDateTime(date);
      output.setAttribute('datetime', date.toISOString());
    } else {
      output.textContent = option?.dataset.timeLabel ?? '';
      output.removeAttribute('datetime');
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
  // A reflow between unhiding and dropping the class lets the row
  // transition run instead of snapping open.
  void form.offsetWidth;
  form.classList.remove('is-collapsed');
  setExpanded(button, true);
  decorateTimes(form);
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

function leaveDiff(form: HTMLFormElement): void {
  for (const select of form.querySelectorAll('select')) {
    if (select instanceof HTMLSelectElement) {
      select.removeAttribute('name');
    }
  }
  form.requestSubmit();
}

function wireForm(form: HTMLFormElement): void {
  if (form.dataset.ghrmCompareWired === '1') return;
  form.dataset.ghrmCompareWired = '1';
  decorateTimes(form);

  form.addEventListener('change', (event) => {
    if (!(event.target instanceof HTMLSelectElement)) return;
    rememberSelection(form);
    decorateTimes(form);
    scheduleSubmit(form);
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
    restoreSelection(form);
    wireForm(form);
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

  tools.prepend(button);
}

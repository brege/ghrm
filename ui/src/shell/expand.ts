export const EXPAND_INPUT = '[data-ghrm-expand-input]';
export const EXPAND_TOGGLE = '[data-ghrm-expand-toggle]';

export interface ExpandFieldOpts {
  root: HTMLElement;
  input: HTMLInputElement;
  toggle: HTMLElement;
  onOpen?: () => void;
  onClose?: () => void;
  onInput?: () => void;
  onEnter?: () => void;
}

export interface ExpandParts {
  input: HTMLInputElement;
  toggle: HTMLElement;
}

// Resolve the input and trailing toggle of an expanding field from its root.
export function expandParts(root: HTMLElement): ExpandParts | null {
  const input = root.querySelector(EXPAND_INPUT);
  const toggle = root.querySelector(EXPAND_TOGGLE);
  if (
    !(input instanceof HTMLInputElement) ||
    !(toggle instanceof HTMLElement)
  ) {
    return null;
  }
  return { input, toggle };
}

// A text field that grows leftward out of a trailing toggle button, shared by
// the topbar path search and the explorer new-file control. The controller owns
// open state, the `is-open` class the CSS transition keys on, `aria-expanded`,
// input reachability, focus handling, and Escape; callers add the behavior that
// runs on typing (onInput) and on Enter (onEnter).
export class ExpandField {
  private readonly root: HTMLElement;
  private readonly opts: ExpandFieldOpts;

  constructor(opts: ExpandFieldOpts) {
    this.opts = opts;
    this.root = opts.root;
    this.bind();
  }

  get input(): HTMLInputElement {
    return this.opts.input;
  }

  get toggle(): HTMLElement {
    return this.opts.toggle;
  }

  get value(): string {
    return this.opts.input.value;
  }

  get open(): boolean {
    return this.root.classList.contains('is-open');
  }

  // Reflect open state without moving focus or running callbacks, for restoring
  // a field after a fragment swap.
  apply(open: boolean): void {
    this.root.classList.toggle('is-open', open);
    this.opts.toggle.setAttribute('aria-expanded', open ? 'true' : 'false');
    this.opts.input.tabIndex = open ? 0 : -1;
  }

  setOpen(open: boolean): void {
    this.apply(open);
    if (open) {
      this.opts.input.focus();
      this.opts.onOpen?.();
      return;
    }
    this.clearInvalid();
    this.opts.onClose?.();
  }

  toggleOpen(): void {
    this.setOpen(!this.open);
  }

  // Keep the field open and mark the entered value as rejected; the message
  // reaches the user through the input's tooltip.
  invalid(message: string): void {
    this.opts.input.setAttribute('aria-invalid', 'true');
    this.opts.input.title = message;
    this.opts.input.focus();
  }

  clearInvalid(): void {
    this.opts.input.removeAttribute('aria-invalid');
    this.opts.input.title = '';
  }

  release(): void {
    this.opts.toggle.onclick = null;
    this.opts.input.oninput = null;
    this.opts.input.onkeydown = null;
  }

  private bind(): void {
    this.opts.toggle.onclick = () => this.toggleOpen();
    this.opts.input.oninput = () => {
      this.clearInvalid();
      this.opts.onInput?.();
    };
    this.opts.input.onkeydown = (event) => this.onKeydown(event);
  }

  private onKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      this.setOpen(false);
      this.opts.toggle.focus();
      return;
    }
    if (event.key === 'Enter' && this.opts.onEnter) {
      event.preventDefault();
      this.opts.onEnter();
    }
  }
}

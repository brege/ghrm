export interface RenameOutcome {
  ok: boolean;
  message?: string;
}

export interface InlineRenameOpts {
  // The element the input is inserted before; hidden elements are restored on
  // close, so pass the visible name link (and any controls) through hide.
  anchor: HTMLElement;
  value: string;
  label: string;
  hide?: HTMLElement[];
  invalidTitle: string;
  errorTitle?: string;
  normalize?: (raw: string) => string;
  validate?: (name: string) => boolean;
  submit: (name: string) => Promise<RenameOutcome>;
}

// Shared inline-rename lifecycle used by the gist stash rows, the explorer
// rows, the explorer new-file control, and the file view's breadcrumb rename:
// swap an input in over the current name, submit on Enter or blur, restore on
// Escape or an unchanged name, and mark the input invalid (keeping it open for
// retry) when validation or the submit callback rejects the name.
export function beginInlineRename(
  opts: InlineRenameOpts,
): HTMLInputElement | null {
  const host = opts.anchor.parentElement;
  if (!host || host.querySelector('[data-ghrm-rename-input]')) return null;

  const input = document.createElement('input');
  input.type = 'text';
  input.className = 'ghrm-row-input';
  input.dataset.ghrmRenameInput = '1';
  input.value = opts.value;
  input.setAttribute('aria-label', opts.label);
  input.autocomplete = 'off';
  input.spellcheck = false;

  const hidden = opts.hide ?? [];
  for (const element of hidden) {
    element.hidden = true;
  }
  host.insertBefore(input, opts.anchor);
  input.focus();
  input.select();

  const normalize = opts.normalize ?? ((raw: string) => raw.trim());
  const original = normalize(opts.value);

  const restore = () => {
    input.remove();
    for (const element of hidden) {
      element.hidden = false;
    }
  };

  const markInvalid = (message: string) => {
    input.setAttribute('aria-invalid', 'true');
    input.title = message;
    input.focus();
  };

  const submit = async () => {
    if (input.dataset.ghrmSaving === '1') return;
    const name = normalize(input.value);
    if (opts.validate && !opts.validate(name)) {
      markInvalid(opts.invalidTitle);
      return;
    }
    if (name === original) {
      restore();
      return;
    }
    input.dataset.ghrmSaving = '1';
    let outcome: RenameOutcome;
    try {
      outcome = await opts.submit(name);
    } catch {
      input.dataset.ghrmSaving = '0';
      markInvalid(opts.errorTitle ?? opts.invalidTitle);
      return;
    }
    if (!outcome.ok) {
      input.dataset.ghrmSaving = '0';
      markInvalid(outcome.message ?? opts.invalidTitle);
      return;
    }
    restore();
  };

  input.addEventListener('keydown', (event) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      void submit();
    } else if (event.key === 'Escape') {
      event.preventDefault();
      restore();
    }
  });
  input.addEventListener('blur', () => {
    if (input.isConnected) {
      void submit();
    }
  });
  return input;
}

// A single ordinary path component: file renames and new files stay inside
// their directory.
export function validFileName(name: string): boolean {
  return (
    name !== '' &&
    name !== '.' &&
    name !== '..' &&
    !name.includes('/') &&
    !name.includes('\\')
  );
}

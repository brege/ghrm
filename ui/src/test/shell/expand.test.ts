// biome-ignore-all lint/style/noNonNullAssertion: test assertions
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ExpandField, expandParts } from '../../shell/expand';

function fixture(): HTMLElement {
  document.body.innerHTML = `
    <div class="ghrm-expand">
      <div class="ghrm-expand-field">
        <input class="ghrm-expand-input" data-ghrm-expand-input type="text" tabindex="-1">
      </div>
      <button class="ghrm-expand-toggle" data-ghrm-expand-toggle type="button" aria-expanded="false"></button>
    </div>`;
  return document.querySelector<HTMLElement>('.ghrm-expand')!;
}

function build(
  root: HTMLElement,
  extra: Partial<ConstructorParameters<typeof ExpandField>[0]> = {},
): ExpandField {
  const parts = expandParts(root)!;
  return new ExpandField({
    root,
    input: parts.input,
    toggle: parts.toggle,
    ...extra,
  });
}

describe('ExpandField', () => {
  let root: HTMLElement;

  beforeEach(() => {
    root = fixture();
  });

  it('reports missing parts instead of binding a partial field', () => {
    document.body.innerHTML = '<div class="ghrm-expand"></div>';
    expect(
      expandParts(document.querySelector<HTMLElement>('.ghrm-expand')!),
    ).toBeNull();
  });

  it('opens from the toggle and makes the input reachable', () => {
    const onOpen = vi.fn();
    const field = build(root, { onOpen });

    field.toggle.click();

    expect(field.open).toBe(true);
    expect(root.classList.contains('is-open')).toBe(true);
    expect(field.toggle.getAttribute('aria-expanded')).toBe('true');
    expect(field.input.tabIndex).toBe(0);
    expect(document.activeElement).toBe(field.input);
    expect(onOpen).toHaveBeenCalledTimes(1);
  });

  it('closes from the toggle and reports the close once', () => {
    const onClose = vi.fn();
    const field = build(root, { onClose });

    field.toggle.click();
    field.toggle.click();

    expect(field.open).toBe(false);
    expect(field.toggle.getAttribute('aria-expanded')).toBe('false');
    expect(field.input.tabIndex).toBe(-1);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('closes on Escape and returns focus to the toggle', () => {
    const field = build(root);
    field.setOpen(true);

    field.input.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }),
    );

    expect(field.open).toBe(false);
    expect(document.activeElement).toBe(field.toggle);
  });

  it('runs the Enter callback and suppresses the default submit', () => {
    const onEnter = vi.fn();
    const field = build(root, { onEnter });
    field.setOpen(true);

    const event = new KeyboardEvent('keydown', {
      key: 'Enter',
      bubbles: true,
      cancelable: true,
    });
    field.input.dispatchEvent(event);

    expect(onEnter).toHaveBeenCalledTimes(1);
    expect(event.defaultPrevented).toBe(true);
  });

  it('clears a rejected value marker on the next keystroke', () => {
    const onInput = vi.fn();
    const field = build(root, { onInput });
    field.invalid('Nope');
    expect(field.input.getAttribute('aria-invalid')).toBe('true');
    expect(field.input.title).toBe('Nope');

    field.input.dispatchEvent(new Event('input'));

    expect(field.input.hasAttribute('aria-invalid')).toBe(false);
    expect(field.input.title).toBe('');
    expect(onInput).toHaveBeenCalledTimes(1);
  });

  it('restores open state without focus or callbacks', () => {
    const onOpen = vi.fn();
    const field = build(root, { onOpen });

    field.apply(true);

    expect(field.open).toBe(true);
    expect(field.input.tabIndex).toBe(0);
    expect(document.activeElement).not.toBe(field.input);
    expect(onOpen).not.toHaveBeenCalled();
  });

  it('stops responding after release', () => {
    const onOpen = vi.fn();
    const field = build(root, { onOpen });

    field.release();
    field.toggle.click();

    expect(field.open).toBe(false);
    expect(onOpen).not.toHaveBeenCalled();
  });
});

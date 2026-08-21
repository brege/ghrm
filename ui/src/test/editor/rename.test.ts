import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  beginInlineRename,
  type RenameOutcome,
  validFileName,
} from '../../editor/rename';

interface Harness {
  host: HTMLElement;
  link: HTMLAnchorElement;
  input: HTMLInputElement;
  submit: ReturnType<typeof vi.fn>;
}

function setup(outcome: RenameOutcome = { ok: true }): Harness {
  document.body.innerHTML =
    '<table><tbody><tr><td><a href="/old.md">old.md</a></td></tr></tbody></table>';
  const host = document.querySelector('td') as HTMLElement;
  const link = document.querySelector('a') as HTMLAnchorElement;
  const submit = vi.fn().mockResolvedValue(outcome);
  const input = beginInlineRename({
    anchor: link,
    value: link.textContent ?? '',
    label: 'File name',
    hide: [link],
    invalidTitle: 'invalid name',
    errorTitle: 'request failed',
    validate: validFileName,
    submit,
  }) as HTMLInputElement;
  return { host, link, input, submit };
}

function pressEnter(input: HTMLInputElement): void {
  input.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }),
  );
}

describe('beginInlineRename', () => {
  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  it('inserts an input seeded with the value and hides the anchors', () => {
    const h = setup();

    expect(h.input.value).toBe('old.md');
    expect(h.input.dataset.ghrmRenameInput).toBe('1');
    expect(h.link.hidden).toBe(true);
  });

  it('refuses a second concurrent rename in the same host', () => {
    const h = setup();
    const second = beginInlineRename({
      anchor: h.link,
      value: 'old.md',
      label: 'File name',
      invalidTitle: 'invalid name',
      submit: vi.fn(),
    });

    expect(second).toBeNull();
    expect(h.host.querySelectorAll('input').length).toBe(1);
  });

  it('Escape restores without submitting', () => {
    const h = setup();
    h.input.value = 'changed.md';
    h.input.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }),
    );

    expect(h.host.querySelector('input')).toBeNull();
    expect(h.link.hidden).toBe(false);
    expect(h.submit).not.toHaveBeenCalled();
  });

  it('an unchanged name restores without submitting', () => {
    const h = setup();
    pressEnter(h.input);

    expect(h.host.querySelector('input')).toBeNull();
    expect(h.submit).not.toHaveBeenCalled();
  });

  it('an invalid name marks the input and keeps it open', () => {
    const h = setup();
    h.input.value = 'a/b.md';
    pressEnter(h.input);

    expect(h.input.getAttribute('aria-invalid')).toBe('true');
    expect(h.input.title).toBe('invalid name');
    expect(h.submit).not.toHaveBeenCalled();
  });

  it('a successful submit closes the input', async () => {
    const h = setup({ ok: true });
    h.input.value = 'new.md';
    pressEnter(h.input);

    await vi.waitUntil(() => h.host.querySelector('input') === null);
    expect(h.submit).toHaveBeenCalledWith('new.md');
    expect(h.link.hidden).toBe(false);
  });

  it('a rejected submit marks the input with the message and allows retry', async () => {
    const h = setup({ ok: false, message: 'taken' });
    h.input.value = 'new.md';
    pressEnter(h.input);

    await vi.waitUntil(() => h.input.getAttribute('aria-invalid') === 'true');
    expect(h.input.title).toBe('taken');

    h.submit.mockResolvedValue({ ok: true });
    h.input.value = 'other.md';
    pressEnter(h.input);
    await vi.waitUntil(() => h.host.querySelector('input') === null);
    expect(h.submit).toHaveBeenCalledTimes(2);
  });

  it('a thrown submit resets the saving state and allows retry', async () => {
    const h = setup();
    h.submit.mockRejectedValueOnce(new Error('offline'));
    h.input.value = 'new.md';
    pressEnter(h.input);

    await vi.waitUntil(() => h.input.getAttribute('aria-invalid') === 'true');
    expect(h.input.title).toBe('request failed');
    expect(h.input.dataset.ghrmSaving).toBe('0');

    h.submit.mockResolvedValue({ ok: true });
    pressEnter(h.input);
    await vi.waitUntil(() => h.host.querySelector('input') === null);
    expect(h.submit).toHaveBeenCalledTimes(2);
  });
});

describe('validFileName', () => {
  it('accepts ordinary names and rejects separators and traversal', () => {
    expect(validFileName('notes.md')).toBe(true);
    expect(validFileName('.gitignore')).toBe(true);
    expect(validFileName('')).toBe(false);
    expect(validFileName('.')).toBe(false);
    expect(validFileName('..')).toBe(false);
    expect(validFileName('a/b')).toBe(false);
    expect(validFileName('a\\b')).toBe(false);
  });
});

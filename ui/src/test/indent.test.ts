import { describe, expect, it } from 'vitest';
import { type IndentEdit, indentEdit } from '../indent';

function applyEdit(text: string, edit: IndentEdit): string {
  return text.slice(0, edit.start) + edit.text + text.slice(edit.end);
}

describe('indentEdit', () => {
  it('inserts spaces at a collapsed caret', () => {
    const edit = indentEdit('alpha', 2, 2, false);

    expect(applyEdit('alpha', edit)).toBe('al  pha');
    expect(edit).toMatchObject({
      start: 2,
      end: 2,
      selectionStart: 4,
      selectionEnd: 4,
    });
  });

  it('indents every selected line', () => {
    const text = 'one\ntwo\nthree';
    const edit = indentEdit(text, 1, 7, false);

    expect(applyEdit(text, edit)).toBe('  one\n  two\nthree');
    expect(edit).toMatchObject({
      start: 0,
      end: 7,
      selectionStart: 3,
      selectionEnd: 11,
    });
  });

  it('does not include the next line when selection ends at a newline', () => {
    const text = 'one\ntwo\nthree';
    const edit = indentEdit(text, 0, 4, false);

    expect(applyEdit(text, edit)).toBe('  one\ntwo\nthree');
    expect(edit).toMatchObject({
      start: 0,
      end: 3,
      selectionStart: 0,
      selectionEnd: 6,
    });
  });

  it('removes supported indentation from every selected line', () => {
    const text = '  one\n two\n\tthree\nfour';
    const edit = indentEdit(text, 0, 17, true);

    expect(applyEdit(text, edit)).toBe('one\ntwo\nthree\nfour');
    expect(edit).toMatchObject({
      start: 0,
      end: 17,
      selectionStart: 0,
      selectionEnd: 13,
    });
  });

  it('moves selection endpoints out of removed indentation', () => {
    const text = '  one';
    const edit = indentEdit(text, 1, 5, true);

    expect(applyEdit(text, edit)).toBe('one');
    expect(edit).toMatchObject({
      start: 0,
      end: 5,
      selectionStart: 0,
      selectionEnd: 3,
    });
  });
});

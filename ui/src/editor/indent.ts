interface LineRange {
  lineStart: number;
  lineEnd: number;
}

interface Removal {
  position: number;
  size: number;
}

interface UndentBlock {
  text: string;
  removals: Removal[];
}

export interface IndentEdit {
  start: number;
  end: number;
  text: string;
  selectionStart: number;
  selectionEnd: number;
}

const indentText = '  ';

// Indent edits operate on whole lines, while selection offsets follow the original caret range.
function selectedLineRange(
  text: string,
  start: number,
  end: number,
): LineRange {
  const lineStart = start === 0 ? 0 : text.lastIndexOf('\n', start - 1) + 1;
  const endRef = end > start && text[end - 1] === '\n' ? end - 1 : end;
  const nextBreak = text.indexOf('\n', endRef);
  const lineEnd = nextBreak === -1 ? text.length : nextBreak;
  return { lineStart, lineEnd };
}

function lineStarts(text: string, start: number, end: number): number[] {
  const starts = [start];
  for (let i = start; i < end; i += 1) {
    if (text[i] === '\n' && i + 1 < end) {
      starts.push(i + 1);
    }
  }
  return starts;
}

function shiftAfterInsert(offset: number, positions: number[]): number {
  return (
    offset +
    positions.filter((position) => position < offset).length * indentText.length
  );
}

function linePrefixLen(line: string): number {
  if (line.startsWith(indentText)) return indentText.length;
  if (line.startsWith('\t') || line.startsWith(' ')) return 1;
  return 0;
}

function shiftAfterRemoval(offset: number, removals: Removal[]): number {
  let next = offset;
  for (const removal of removals) {
    if (offset >= removal.position + removal.size) {
      next -= removal.size;
    } else if (offset > removal.position) {
      next -= offset - removal.position;
    }
  }
  return next;
}

function undentBlock(text: string, start: number, end: number): UndentBlock {
  const lines = text.slice(start, end).split('\n');
  const removals: Removal[] = [];
  const out: string[] = [];
  let position = start;

  for (const line of lines) {
    const size = linePrefixLen(line);
    if (size > 0) {
      removals.push({ position, size });
    }
    out.push(line.slice(size));
    position += line.length + 1;
  }

  return { text: out.join('\n'), removals };
}

export function indentEdit(
  text: string,
  start: number,
  end: number,
  outdent: boolean,
): IndentEdit {
  if (!outdent && start === end) {
    return {
      start,
      end,
      text: indentText,
      selectionStart: start + indentText.length,
      selectionEnd: start + indentText.length,
    };
  }

  const { lineStart, lineEnd } = selectedLineRange(text, start, end);
  if (outdent) {
    const block = undentBlock(text, lineStart, lineEnd);
    return {
      start: lineStart,
      end: lineEnd,
      text: block.text,
      selectionStart: shiftAfterRemoval(start, block.removals),
      selectionEnd: shiftAfterRemoval(end, block.removals),
    };
  }

  const starts = lineStarts(text, lineStart, lineEnd);
  return {
    start: lineStart,
    end: lineEnd,
    text: text
      .slice(lineStart, lineEnd)
      .split('\n')
      .map((line) => `${indentText}${line}`)
      .join('\n'),
    selectionStart: shiftAfterInsert(start, starts),
    selectionEnd: shiftAfterInsert(end, starts),
  };
}

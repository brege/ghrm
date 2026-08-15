import { escapeHtml, qselAll } from '../dom';

const SHELL_BUILTINS = new Set([
  '.',
  ':',
  'alias',
  'bg',
  'bind',
  'break',
  'builtin',
  'caller',
  'cd',
  'command',
  'compgen',
  'complete',
  'compopt',
  'continue',
  'declare',
  'dirs',
  'disown',
  'echo',
  'enable',
  'eval',
  'exec',
  'exit',
  'export',
  'false',
  'fc',
  'fg',
  'getopts',
  'hash',
  'help',
  'history',
  'jobs',
  'kill',
  'let',
  'local',
  'logout',
  'mapfile',
  'popd',
  'printf',
  'pushd',
  'pwd',
  'read',
  'readarray',
  'readonly',
  'return',
  'set',
  'shift',
  'shopt',
  'source',
  'suspend',
  'test',
  'times',
  'trap',
  'true',
  'type',
  'typeset',
  'ulimit',
  'umask',
  'unalias',
  'unset',
  'wait',
]);
let customLanguagesRegistered = false;

export function renderCode(): void {
  if (typeof window.hljs?.highlightElement !== 'function') {
    return;
  }
  registerCustomLanguages();

  for (const code of qselAll('.markdown-body pre code')) {
    const hasLanguage = [...code.classList].some((name) =>
      name.startsWith('language-'),
    );
    if (!hasLanguage) {
      continue;
    }
    if (code.dataset.ghrmHighlighted === '1') {
      continue;
    }
    window.hljs.highlightElement(code);
    normalizeShellHighlight(code);
    code.dataset.ghrmHighlighted = '1';
  }
}

function highlightBlobCode(code: HTMLElement): void {
  if (code.dataset.ghrmHighlighted === '1') {
    return;
  }

  const hasLanguage = [...code.classList].some((name) =>
    name.startsWith('language-'),
  );
  if (!hasLanguage || typeof window.hljs?.highlightElement !== 'function') {
    return;
  }

  registerCustomLanguages();
  window.hljs.highlightElement(code);
  normalizeShellHighlight(code);
  code.dataset.ghrmHighlighted = '1';
}

function registerCustomLanguages(): void {
  if (customLanguagesRegistered) {
    return;
  }
  if (typeof window.hljs?.registerLanguage !== 'function') {
    return;
  }
  window.hljs.registerLanguage('just', justLanguage);
  customLanguagesRegistered = true;
}

function justLanguage(): Record<string, unknown> {
  return {
    name: 'Just',
    aliases: ['justfile'],
    keywords: {
      keyword: 'alias export if import mod set shell tempfile unstable',
      literal: 'false true',
    },
    contains: [
      { className: 'comment', begin: /#/, end: /$/ },
      {
        className: 'section',
        begin:
          /^[A-Za-z_][\w-]*(?:::[A-Za-z_][\w-]*)*(?:\s+[A-Za-z_][\w-]*)*\s*:/,
      },
      {
        className: 'attr',
        begin: /^[A-Za-z_][\w-]*\s*(?::=|\+=|=\s)/,
      },
      {
        className: 'variable',
        begin: /\{\{/,
        end: /\}\}/,
      },
      {
        begin: /^[ \t]+/,
        end: /$/,
        subLanguage: 'shell',
      },
      {
        className: 'string',
        variants: [
          {
            begin: /"/,
            end: /"/,
            contains: [{ begin: /\\./ }],
          },
          {
            begin: /'/,
            end: /'/,
            contains: [{ begin: /\\./ }],
          },
        ],
      },
    ],
  };
}

function openTag(node: Element): string {
  const attrs = [...node.attributes]
    .map((attr) => `${attr.name}="${escapeHtml(attr.value)}"`)
    .join(' ');
  return attrs
    ? `<${node.tagName.toLowerCase()} ${attrs}>`
    : `<${node.tagName.toLowerCase()}>`;
}

function pushHighlightedNode(
  node: Node,
  lines: string[],
  stack: Element[],
): void {
  if (node.nodeType === Node.TEXT_NODE) {
    const parts = (node.textContent || '').split('\n');
    for (let idx = 0; idx < parts.length; idx += 1) {
      if (idx > 0) {
        for (let rev = stack.length - 1; rev >= 0; rev -= 1) {
          lines[lines.length - 1] += `</${stack[rev].tagName.toLowerCase()}>`;
        }
        lines.push('');
        for (const el of stack) {
          lines[lines.length - 1] += openTag(el);
        }
      }
      lines[lines.length - 1] += escapeHtml(parts[idx]);
    }
    return;
  }

  if (node.nodeType !== Node.ELEMENT_NODE) {
    return;
  }

  const el = node as Element;
  lines[lines.length - 1] += openTag(el);
  stack.push(el);
  for (const child of el.childNodes) {
    pushHighlightedNode(child, lines, stack);
  }
  stack.pop();
  lines[lines.length - 1] += `</${el.tagName.toLowerCase()}>`;
}

interface DiffLineNumber {
  old: number | null;
  new: number | null;
}

// Unified hunk headers define the old and new cursors. Context advances both
// cursors, deletions advance only the old cursor, and additions advance only
// the new cursor. Patch metadata and no-newline markers have no source line.
function diffLineNumbers(lines: string[]): DiffLineNumber[] {
  let oldLine: number | null = null;
  let newLine: number | null = null;
  // The optional counts do not affect cursor initialization.
  const hunk = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/;

  return lines.map((line) => {
    if (line.startsWith('diff --git ')) {
      oldLine = null;
      newLine = null;
      return { old: null, new: null };
    }
    const header = hunk.exec(line);
    if (header) {
      oldLine = Number(header[1]);
      newLine = Number(header[2]);
      return { old: null, new: null };
    }
    if (oldLine === null || newLine === null) {
      return { old: null, new: null };
    }
    if (line.startsWith('\\ No newline at end of file')) {
      return { old: null, new: null };
    }
    if (line.startsWith('-')) {
      const current = oldLine;
      oldLine += 1;
      return { old: current, new: null };
    }
    if (line.startsWith('+')) {
      const current = newLine;
      newLine += 1;
      return { old: null, new: current };
    }
    if (line.startsWith(' ')) {
      const current = { old: oldLine, new: newLine };
      oldLine += 1;
      newLine += 1;
      return current;
    }
    return { old: null, new: null };
  });
}

function diffLineCell(side: 'old' | 'new', line: number | null): string {
  const cls = `ghrm-blob-line-no ghrm-blob-line-no-${side}`;
  if (line === null) {
    return `<td class="${cls}" aria-hidden="true"></td>`;
  }
  return `<td class="${cls}" data-line-number="${line}"><span class="ghrm-blob-line-no-text">${line}</span></td>`;
}

function renderBlob(block: Element): void {
  const code = block.querySelector('.ghrm-blob-source code');
  const body = block.querySelector('.ghrm-blob-table tbody');
  if (!(code instanceof HTMLElement) || !body) {
    return;
  }

  highlightBlobCode(code);

  const source = code.textContent ?? '';
  if (source === '') {
    body.innerHTML = '';
    return;
  }

  const lines = [''];
  for (const child of code.childNodes) {
    pushHighlightedNode(child, lines, []);
  }

  // A terminating newline closes the last line; the empty segment it leaves
  // behind is not a line of its own. Drop exactly that segment, and mark files
  // that lack the terminator the way git does, so the line model stays faithful
  // either way.
  const terminated = source.endsWith('\n');
  if (terminated && lines.length > 1 && lines[lines.length - 1] === '') {
    lines.pop();
  }

  const diffBlob = code.classList.contains('language-diff');
  block.classList.toggle('ghrm-blob-diff', diffBlob);
  const plainLines = source.split('\n');
  if (
    terminated &&
    plainLines.length > 1 &&
    plainLines[plainLines.length - 1] === ''
  ) {
    plainLines.pop();
  }
  const diffNumbers = diffBlob ? diffLineNumbers(plainLines) : [];

  const rows = lines.map((line, idx) => {
    const content = line || '&#8203;';
    const lineNo = idx + 1;
    const rowClass = diffBlob ? diffRowClass(plainLines[idx] ?? '') : '';
    const rowAttr = rowClass ? ` class="${rowClass}"` : '';
    const gutter = diffBlob
      ? `${diffLineCell('old', diffNumbers[idx]?.old ?? null)}${diffLineCell('new', diffNumbers[idx]?.new ?? null)}`
      : `<td class="ghrm-blob-line-no" data-line-number="${lineNo}"><span class="ghrm-blob-line-no-text">${lineNo}</span></td>`;
    return `<tr${rowAttr}>${gutter}<td class="ghrm-blob-line-code"><code class="ghrm-blob-line-text">${content}</code></td></tr>`;
  });

  if (!terminated) {
    const gutter = diffBlob
      ? `${diffLineCell('old', null)}${diffLineCell('new', null)}`
      : '<td class="ghrm-blob-line-no" aria-hidden="true"></td>';
    rows.push(
      `<tr class="ghrm-blob-eof-row">${gutter}<td class="ghrm-blob-line-code"><span class="ghrm-blob-eof" title="No newline at end of file">No newline at end of file</span></td></tr>`,
    );
  }

  body.innerHTML = rows.join('');
}

// Unified diff row classes come from the raw first characters so whole
// rows, including the number gutter, carry add, delete, and hunk tinting;
// the +++/--- file header lines stay untinted.
function diffRowClass(line: string): string {
  if (line.startsWith('@@')) return 'ghrm-blob-row-hunk';
  if (line.startsWith('+') && !line.startsWith('+++')) {
    return 'ghrm-blob-row-add';
  }
  if (line.startsWith('-') && !line.startsWith('---')) {
    return 'ghrm-blob-row-del';
  }
  return '';
}

export function renderBlobs(): void {
  for (const block of document.querySelectorAll('.ghrm-blob')) {
    renderBlob(block);
  }
}

function isShellCode(code: Element): boolean {
  return [...code.classList].some((name) =>
    ['language-bash', 'language-sh', 'language-shell'].includes(name),
  );
}

function normalizeShellHighlight(code: Element): void {
  if (!isShellCode(code)) {
    return;
  }

  for (const node of code.querySelectorAll('.hljs-built_in')) {
    if (SHELL_BUILTINS.has(node.textContent?.trim() || '')) {
      continue;
    }
    node.replaceWith(document.createTextNode(node.textContent || ''));
  }
}

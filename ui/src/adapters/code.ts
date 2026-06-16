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

function renderBlob(block: Element): void {
  const code = block.querySelector('.ghrm-blob-source code');
  const body = block.querySelector('.ghrm-blob-table tbody');
  if (!(code instanceof HTMLElement) || !body) {
    return;
  }

  highlightBlobCode(code);

  const lines = [''];
  for (const child of code.childNodes) {
    pushHighlightedNode(child, lines, []);
  }

  body.innerHTML = lines
    .map((line, idx) => {
      const content = line || '&#8203;';
      const lineNo = idx + 1;
      return `<tr><td class="ghrm-blob-line-no" data-line-number="${lineNo}"><span class="ghrm-blob-line-no-text">${lineNo}</span></td><td class="ghrm-blob-line-code"><code class="ghrm-blob-line-text">${content}</code></td></tr>`;
    })
    .join('');
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

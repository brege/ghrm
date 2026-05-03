import { hasFeature } from '../vendor.js';

function restoreGitHubInlineMath(container) {
  for (const code of container.querySelectorAll('code')) {
    if (code.closest('pre')) {
      continue;
    }

    const previous = code.previousSibling;
    const next = code.nextSibling;
    if (
      previous?.nodeType !== Node.TEXT_NODE ||
      next?.nodeType !== Node.TEXT_NODE
    ) {
      continue;
    }

    const before = previous.textContent || '';
    const after = next.textContent || '';
    if (!before.endsWith('$') || !after.startsWith('$')) {
      continue;
    }

    const math = document.createTextNode(
      `${before.slice(0, -1)}$${code.textContent || ''}$${after.slice(1)}`,
    );
    code.replaceWith(math);
    previous.textContent = before.slice(0, -1);
    next.textContent = after.slice(1);
    previous.remove();
    next.remove();
  }
}

export async function renderMath() {
  if (!hasFeature('math')) return;

  const containers = document.querySelectorAll('.markdown-body');
  if (containers.length === 0) return;

  if (typeof window.renderMathInElement !== 'function') return;

  for (const container of containers) {
    // GitHub's $`...`$ form becomes $<code>...</code>$ after Markdown parsing.
    restoreGitHubInlineMath(container);
    window.renderMathInElement(container, {
      delimiters: [
        { left: '$$', right: '$$', display: true },
        { left: '$`', right: '`$', display: false },
        { left: '$', right: '$', display: false },
        { left: '\\(', right: '\\)', display: false },
        { left: '\\[', right: '\\]', display: true },
      ],
      throwOnError: false,
      ignoredTags: ['script', 'noscript', 'style', 'textarea', 'pre', 'code'],
    });
  }
}

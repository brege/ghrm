import { bindCopy, type CopyButton } from './adapters/copy';
import { icon } from './dom';

type PathTarget = {
  cell: Element;
  path: string;
};

const buttonSelector = '.ghrm-nav-copy-path';
const rowSelector = '.ghrm-nav-table tbody tr';

export function copyPathFromHref(href: string): string {
  const path = new URL(href, location.origin).pathname;
  const rel = decodeURIComponent(path).replace(/^\/+/, '').replace(/\/+$/, '');
  return rel || '.';
}

export function setupPathCopy(root: ParentNode = document): void {
  for (const row of root.querySelectorAll(rowSelector)) {
    addPathCopy(row);
  }
}

function addPathCopy(row: Element): void {
  const target = pathTarget(row);
  if (!target) return;

  target.cell.append(pathButton(target.path));
}

function pathTarget(row: Element): PathTarget | null {
  const cell = row.querySelector('.ghrm-nav-icon');
  const link = row.querySelector<HTMLAnchorElement>('.ghrm-nav-name a');
  if (!cell || !link || cell.querySelector(buttonSelector)) return null;
  if (link.textContent?.trim() === '..') return null;
  if (!cell.querySelector('svg')) return null;

  return { cell, path: copyPathFromHref(link.href) };
}

function pathButton(path: string): CopyButton {
  const label = `Copy path: ${path}`;
  const button = document.createElement('button') as CopyButton;
  button.type = 'button';
  button.className = 'ghrm-nav-copy-path';
  button.dataset.copyLabel = label;
  button.dataset.copyFeedback = 'Copied path';
  button.setAttribute('aria-label', label);
  button.title = label;
  button.innerHTML = `${icon('copy-path', 'ghrm-nav-copy-icon ghrm-copy-icon-copy')}${icon('check', 'ghrm-nav-copy-icon ghrm-copy-icon-check')}`;
  bindCopy(button, path);
  return button;
}

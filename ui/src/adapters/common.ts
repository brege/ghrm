export function getSource(block: Element): string {
  const data = block.querySelector('.ghrm-data');
  if (data instanceof HTMLTemplateElement) {
    return data.content?.textContent?.trim() || '';
  }
  return '';
}

export function isDarkTheme(): boolean {
  return document.documentElement.getAttribute('data-theme') === 'dark';
}

export function setError(block: Element, message: string): void {
  let node = block.querySelector('.ghrm-error');
  if (!node) {
    node = document.createElement('p');
    node.className = 'ghrm-error';
    block.appendChild(node);
  }
  (node as HTMLElement).hidden = false;
  node.textContent = message;
}

export function clearError(block: Element): void {
  const node = block.querySelector('.ghrm-error');
  if (node instanceof HTMLElement) {
    node.hidden = true;
    node.textContent = '';
  }
}

export interface ThemeColors {
  polygon: string;
  polygonFill: string;
  line: string;
  point: string;
}

export function themeColors(): ThemeColors {
  return {
    polygon: '#6f42c1',
    polygonFill: '#6f42c1',
    line: '#0969da',
    point: '#0969da',
  };
}

export function getSource(block) {
  return block.querySelector('.ghrm-data')?.content?.textContent?.trim() || '';
}

export function isDarkTheme() {
  return document.documentElement.getAttribute('data-theme') === 'dark';
}

export function setError(block, message) {
  let node = block.querySelector('.ghrm-error');
  if (!node) {
    node = document.createElement('p');
    node.className = 'ghrm-error';
    block.appendChild(node);
  }
  node.hidden = false;
  node.textContent = message;
}

export function clearError(block) {
  const node = block.querySelector('.ghrm-error');
  if (node) {
    node.hidden = true;
    node.textContent = '';
  }
}

export function themeColors() {
  return {
    polygon: '#6f42c1',
    polygonFill: '#6f42c1',
    line: '#0969da',
    point: '#0969da',
  };
}

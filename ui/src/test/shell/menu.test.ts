import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '../../shell/menu';
import type { GhrmMenus } from '../../shell/menu';

function fixture(): void {
  document.body.innerHTML = `
    <button data-ghrm-menu-toggle aria-controls="filter-menu" aria-expanded="false">Filter</button>
    <div id="filter-menu" data-ghrm-menu-panel hidden>
      <a href="/filter" role="menuitemcheckbox">Filter option</a>
      <button type="button" data-ghrm-menu-disclosure aria-controls="branches" aria-expanded="false">Branches</button>
      <div id="branches" hidden><button type="button" role="menuitemradio">main</button></div>
    </div>
    <button data-ghrm-menu-toggle aria-controls="column-menu" aria-expanded="false">Columns</button>
    <div id="column-menu" data-ghrm-menu-panel hidden>
      <button type="button" role="menuitemcheckbox">Date</button>
    </div>
    <ghrm-menus></ghrm-menus>
  `;
}

describe('ghrm-menus', () => {
  let element: GhrmMenus;

  beforeEach(async () => {
    fixture();
    const found = document.querySelector<GhrmMenus>('ghrm-menus');
    if (!found) throw new Error('missing ghrm-menus');
    element = found;
    await element.updateComplete;
  });

  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  it('opens one panel at a time and synchronizes expanded state', () => {
    const toggles = document.querySelectorAll<HTMLButtonElement>(
      '[data-ghrm-menu-toggle]',
    );
    const filter = document.getElementById('filter-menu') as HTMLElement;
    const columns = document.getElementById('column-menu') as HTMLElement;

    toggles[0].click();
    expect(filter.hidden).toBe(false);
    expect(toggles[0].getAttribute('aria-expanded')).toBe('true');

    toggles[1].click();
    expect(filter.hidden).toBe(true);
    expect(columns.hidden).toBe(false);
    expect(toggles[0].getAttribute('aria-expanded')).toBe('false');
    expect(toggles[1].getAttribute('aria-expanded')).toBe('true');
  });

  it('closes an open panel when its toggle is activated again', () => {
    const toggle = document.querySelector<HTMLButtonElement>(
      '[data-ghrm-menu-toggle]',
    )!;
    const panel = document.getElementById('filter-menu') as HTMLElement;

    toggle.click();
    toggle.click();

    expect(panel.hidden).toBe(true);
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
  });

  it('toggles nested disclosures without closing the parent panel', () => {
    const toggle = document.querySelector<HTMLButtonElement>(
      '[data-ghrm-menu-toggle]',
    )!;
    const disclosure = document.querySelector<HTMLButtonElement>(
      '[data-ghrm-menu-disclosure]',
    )!;
    const panel = document.getElementById('filter-menu') as HTMLElement;
    const section = document.getElementById('branches') as HTMLElement;

    toggle.click();
    disclosure.click();

    expect(panel.hidden).toBe(false);
    expect(section.hidden).toBe(false);
    expect(disclosure.getAttribute('aria-expanded')).toBe('true');

    disclosure.click();
    expect(section.hidden).toBe(true);
    expect(disclosure.getAttribute('aria-expanded')).toBe('false');
  });

  it('reveals the disclosure holding the checked option on open', () => {
    const host = document.createElement('div');
    host.innerHTML = `
      <button data-ghrm-menu-toggle aria-controls="rev-menu" aria-expanded="false">Head</button>
      <div id="rev-menu" data-ghrm-menu-panel hidden>
        <button type="button" role="menuitemradio" aria-checked="false">Working tree</button>
        <button type="button" data-ghrm-menu-disclosure aria-controls="rev-branches" aria-expanded="false">Branches</button>
        <div id="rev-branches" role="group" hidden>
          <button type="button" role="menuitemradio" aria-checked="false">main</button>
        </div>
        <button type="button" data-ghrm-menu-disclosure aria-controls="rev-hashes" aria-expanded="false">Hashes</button>
        <div id="rev-hashes" role="group" hidden>
          <button type="button" role="menuitemradio" aria-checked="true">abc1234</button>
        </div>
      </div>
    `;
    element.before(host);
    const toggle = host.querySelector<HTMLButtonElement>(
      '[data-ghrm-menu-toggle]',
    )!;
    const branches = host.querySelector('#rev-branches') as HTMLElement;
    const hashes = host.querySelector('#rev-hashes') as HTMLElement;
    const branchesToggle = host.querySelector<HTMLElement>(
      '[aria-controls="rev-branches"]',
    )!;
    const hashesToggle = host.querySelector<HTMLElement>(
      '[aria-controls="rev-hashes"]',
    )!;

    toggle.click();

    expect(hashes.hidden).toBe(false);
    expect(hashesToggle.getAttribute('aria-expanded')).toBe('true');
    expect(branches.hidden).toBe(true);
    expect(branchesToggle.getAttribute('aria-expanded')).toBe('false');
  });

  it('closes after a menu item without preventing its default action', () => {
    const toggle = document.querySelector<HTMLButtonElement>(
      '[data-ghrm-menu-toggle]',
    )!;
    const panel = document.getElementById('filter-menu') as HTMLElement;
    const item = panel.querySelector('[role="menuitemcheckbox"]')!;
    const event = new MouseEvent('click', { bubbles: true, cancelable: true });
    const preventDefault = vi.spyOn(event, 'preventDefault');

    toggle.click();
    item.dispatchEvent(event);

    expect(panel.hidden).toBe(true);
    expect(preventDefault).not.toHaveBeenCalled();
  });

  it('closes on outside click and leaves non-item panel clicks open', () => {
    const toggle = document.querySelector<HTMLButtonElement>(
      '[data-ghrm-menu-toggle]',
    )!;
    const panel = document.getElementById('filter-menu') as HTMLElement;

    toggle.click();
    panel.click();
    expect(panel.hidden).toBe(false);

    document.body.click();
    expect(panel.hidden).toBe(true);
  });

  it('closes on Escape and restores focus to the active toggle', () => {
    const toggle = document.querySelector<HTMLButtonElement>(
      '[data-ghrm-menu-toggle]',
    )!;
    const panel = document.getElementById('filter-menu') as HTMLElement;

    toggle.click();
    document.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }),
    );

    expect(panel.hidden).toBe(true);
    expect(document.activeElement).toBe(toggle);
  });

  it('closes on the shared menu close event', () => {
    const toggle = document.querySelector<HTMLButtonElement>(
      '[data-ghrm-menu-toggle]',
    )!;
    const panel = document.getElementById('filter-menu') as HTMLElement;

    toggle.click();
    panel.dispatchEvent(new CustomEvent('ghrm:menu-close', { bubbles: true }));

    expect(panel.hidden).toBe(true);
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
  });

  it('repositions the active panel on resize', () => {
    const toggle = document.querySelector<HTMLButtonElement>(
      '[data-ghrm-menu-toggle]',
    )!;
    const panel = document.getElementById('filter-menu') as HTMLElement;
    vi.spyOn(toggle, 'getBoundingClientRect').mockReturnValue({
      bottom: 100,
      right: 200,
      top: 80,
      left: 160,
      width: 40,
      height: 20,
      x: 160,
      y: 80,
      toJSON: () => {},
    });

    toggle.click();
    window.dispatchEvent(new Event('resize'));

    expect(panel.style.top).toBe('108px');
  });

  it('controls panels inserted after the singleton island connects', () => {
    const host = document.createElement('div');
    host.innerHTML = `
      <button data-ghrm-menu-toggle aria-controls="dynamic-menu" aria-expanded="false">Refs</button>
      <div id="dynamic-menu" data-ghrm-menu-panel hidden></div>
    `;
    element.before(host);
    const toggle = host.querySelector('button')!;
    const panel = host.querySelector('div')!;

    toggle.click();

    expect(panel.hidden).toBe(false);
    expect(toggle.getAttribute('aria-expanded')).toBe('true');
  });

  it('removes global listeners when disconnected', () => {
    const toggle = document.querySelector<HTMLButtonElement>(
      '[data-ghrm-menu-toggle]',
    )!;
    const panel = document.getElementById('filter-menu') as HTMLElement;

    toggle.click();
    element.remove();
    document.body.click();

    expect(panel.hidden).toBe(false);
  });
});

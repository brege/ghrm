import { LitElement } from 'lit';
import { asElement, positionFloatingPanel } from '../dom';

const TOGGLE_SELECTOR = '[data-ghrm-menu-toggle][aria-controls]';
const PANEL_SELECTOR = '[data-ghrm-menu-panel]';
const DISCLOSURE_SELECTOR = '[data-ghrm-menu-disclosure][aria-controls]';
const CLOSE_EVENT = 'ghrm:menu-close';

interface Menu {
  toggle: HTMLElement;
  panel: HTMLElement;
}

function controlledElement(toggle: Element): HTMLElement | null {
  const id = toggle.getAttribute('aria-controls');
  if (!id) return null;
  const panel = document.getElementById(id);
  return panel instanceof HTMLElement ? panel : null;
}

function menuForToggle(toggle: HTMLElement): Menu | null {
  const panel = controlledElement(toggle);
  return panel?.matches(PANEL_SELECTOR) ? { toggle, panel } : null;
}

function menuToggles(): HTMLElement[] {
  return [...document.querySelectorAll(TOGGLE_SELECTOR)].filter(
    (toggle): toggle is HTMLElement => toggle instanceof HTMLElement,
  );
}

function menuPanels(): HTMLElement[] {
  return [...document.querySelectorAll(PANEL_SELECTOR)].filter(
    (panel): panel is HTMLElement => panel instanceof HTMLElement,
  );
}

export class GhrmMenus extends LitElement {
  private active: Menu | null = null;

  protected createRenderRoot(): HTMLElement {
    return this;
  }

  connectedCallback(): void {
    super.connectedCallback();
    document.addEventListener('click', this.handleClick);
    document.addEventListener('keydown', this.handleKey);
    document.addEventListener(CLOSE_EVENT, this.handleClose);
    window.addEventListener('resize', this.handleResize);
    this.closeAll();
  }

  disconnectedCallback(): void {
    super.disconnectedCallback();
    document.removeEventListener('click', this.handleClick);
    document.removeEventListener('keydown', this.handleKey);
    document.removeEventListener(CLOSE_EVENT, this.handleClose);
    window.removeEventListener('resize', this.handleResize);
    this.active = null;
  }

  private setClosed(panel: HTMLElement): void {
    panel.hidden = true;
    for (const toggle of menuToggles()) {
      if (controlledElement(toggle) === panel) {
        toggle.setAttribute('aria-expanded', 'false');
      }
    }
  }

  private closeAll(): void {
    for (const panel of menuPanels()) {
      this.setClosed(panel);
    }
    this.active = null;
  }

  private open(menu: Menu): void {
    this.closeAll();
    menu.panel.hidden = false;
    menu.toggle.setAttribute('aria-expanded', 'true');
    this.revealSelectedDisclosure(menu.panel);
    const width = Number(menu.panel.dataset.ghrmMenuWidth);
    positionFloatingPanel(
      menu.panel,
      menu.toggle,
      Number.isFinite(width) && width > 0 ? width : undefined,
    );
    this.active = menu;
  }

  // On open, expand the disclosure whose section holds the checked option so a
  // menu reopened after a branch, tag, or hash selection shows that group and
  // collapses the others.
  private revealSelectedDisclosure(panel: HTMLElement): void {
    for (const toggle of panel.querySelectorAll(DISCLOSURE_SELECTOR)) {
      if (!(toggle instanceof HTMLElement)) continue;
      const section = controlledElement(toggle);
      if (!section) continue;
      const expanded = section.querySelector('[aria-checked="true"]') !== null;
      toggle.setAttribute('aria-expanded', expanded ? 'true' : 'false');
      section.hidden = !expanded;
    }
  }

  private toggleDisclosure(toggle: HTMLElement): void {
    const panel = controlledElement(toggle);
    if (!panel) return;
    const expanded = toggle.getAttribute('aria-expanded') !== 'true';
    toggle.setAttribute('aria-expanded', expanded ? 'true' : 'false');
    panel.hidden = !expanded;
  }

  private handleClick = (event: MouseEvent): void => {
    const target = asElement(event.target);
    if (!target) return;

    const disclosure = target.closest(DISCLOSURE_SELECTOR);
    if (disclosure instanceof HTMLElement) {
      this.toggleDisclosure(disclosure);
      return;
    }

    const toggle = target.closest(TOGGLE_SELECTOR);
    if (toggle instanceof HTMLElement) {
      const menu = menuForToggle(toggle);
      if (!menu) return;
      if (menu.panel.hidden) {
        this.open(menu);
      } else {
        this.closeAll();
      }
      return;
    }

    const panel = target.closest(PANEL_SELECTOR);
    if (panel instanceof HTMLElement) {
      if (
        target.closest(
          '[data-ghrm-menu-close], [role="menuitem"], [role="menuitemcheckbox"], [role="menuitemradio"]',
        )
      ) {
        this.closeAll();
      }
      return;
    }

    this.closeAll();
  };

  private handleKey = (event: KeyboardEvent): void => {
    if (event.key !== 'Escape' || !this.active) return;
    const toggle = this.active.toggle;
    this.closeAll();
    toggle.focus();
  };

  private handleClose = (): void => {
    this.closeAll();
  };

  private handleResize = (): void => {
    if (!this.active || this.active.panel.hidden) return;
    const width = Number(this.active.panel.dataset.ghrmMenuWidth);
    positionFloatingPanel(
      this.active.panel,
      this.active.toggle,
      Number.isFinite(width) && width > 0 ? width : undefined,
    );
  };
}

declare global {
  interface HTMLElementTagNameMap {
    'ghrm-menus': GhrmMenus;
  }
}

customElements.define('ghrm-menus', GhrmMenus);

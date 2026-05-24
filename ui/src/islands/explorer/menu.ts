import { LitElement } from 'lit';
import { positionFloatingPanel, qselAllFrom } from '../../dom';
import type { GhrmArchiveProgress } from '../archive/progress';

interface MenuConfig {
  name: string;
  toggleId: string;
  panelId: string;
}

interface Menu extends MenuConfig {
  toggle: HTMLElement;
  panel: HTMLElement;
}

const MENU_CONFIGS: MenuConfig[] = [
  {
    name: 'filter',
    toggleId: 'ghrm-view-menu-toggle',
    panelId: 'ghrm-view-menu',
  },
  {
    name: 'sort',
    toggleId: 'ghrm-sort-menu-toggle',
    panelId: 'ghrm-sort-menu',
  },
  {
    name: 'archive',
    toggleId: 'ghrm-archive-menu-toggle',
    panelId: 'ghrm-archive-menu',
  },
  {
    name: 'column',
    toggleId: 'ghrm-column-menu-toggle',
    panelId: 'ghrm-column-menu',
  },
];

export class GhrmExplorerMenus extends LitElement {
  private boundClickHandler: ((e: MouseEvent) => void) | null = null;
  private boundKeyHandler: ((e: KeyboardEvent) => void) | null = null;
  private boundResizeHandler: (() => void) | null = null;
  private connectedOnce = false;

  protected createRenderRoot(): HTMLElement {
    return this;
  }

  connectedCallback(): void {
    super.connectedCallback();
    this.setupMenus();
    if (!this.connectedOnce) {
      this.connectedOnce = true;
      this.addGlobalListeners();
    }
  }

  disconnectedCallback(): void {
    super.disconnectedCallback();
    this.removeGlobalListeners();
    this.connectedOnce = false;
  }

  private getMenus(): Menu[] {
    return MENU_CONFIGS.map((config) => ({
      ...config,
      toggle: document.getElementById(config.toggleId),
      panel: document.getElementById(config.panelId),
    })).filter((m): m is Menu => m.toggle !== null && m.panel !== null);
  }

  private getMenu(name: string): Menu | null {
    return this.getMenus().find((m) => m.name === name) ?? null;
  }

  private hasAllMenus(): boolean {
    return this.getMenus().length === MENU_CONFIGS.length;
  }

  private closeAllMenus(): void {
    for (const { toggle, panel } of this.getMenus()) {
      panel.hidden = true;
      toggle.setAttribute('aria-expanded', 'false');
    }
  }

  private openMenu(name: string): void {
    const menu = this.getMenu(name);
    if (!menu) return;
    this.closeAllMenus();
    menu.panel.hidden = false;
    menu.toggle.setAttribute('aria-expanded', 'true');
    positionFloatingPanel(menu.panel, menu.toggle);
  }

  private setupMenus(): void {
    const menus = this.getMenus();
    if (menus.length === 0) return;

    this.closeAllMenus();

    for (const menu of menus) {
      menu.toggle.onclick = () => {
        if (menu.panel.hidden) {
          this.openMenu(menu.name);
        } else {
          this.closeAllMenus();
        }
      };

      for (const option of qselAllFrom(menu.panel, '.ghrm-view-option')) {
        option.onclick = (event) => {
          this.closeAllMenus();
          const archiveUrl = option.dataset.ghrmArchiveUrl;
          if (archiveUrl) {
            event.preventDefault();
            const progress = document.querySelector<GhrmArchiveProgress>(
              'ghrm-archive-progress',
            );
            progress?.startJob(archiveUrl);
          }
        };
      }
    }
  }

  private addGlobalListeners(): void {
    this.boundClickHandler = (e: MouseEvent) => {
      if (!this.hasAllMenus()) return;
      const target = e.target instanceof Node ? e.target : null;
      if (!target) return;
      const dirToggle = document.getElementById('ghrm-sort-dir-toggle');
      const insideMenu = this.getMenus().some(
        ({ toggle, panel }) =>
          toggle.contains(target) || panel.contains(target),
      );
      if (insideMenu || dirToggle?.contains(target)) return;
      this.closeAllMenus();
    };

    this.boundKeyHandler = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (!this.hasAllMenus()) return;
      const openMenu = this.getMenus().find(({ panel }) => !panel.hidden);
      if (openMenu) {
        this.closeAllMenus();
        openMenu.toggle.focus();
      }
    };

    this.boundResizeHandler = () => {
      if (!this.hasAllMenus()) return;
      for (const { toggle, panel } of this.getMenus()) {
        if (!panel.hidden) {
          positionFloatingPanel(panel, toggle);
        }
      }
    };

    document.addEventListener('click', this.boundClickHandler);
    document.addEventListener('keydown', this.boundKeyHandler);
    window.addEventListener('resize', this.boundResizeHandler);
  }

  private removeGlobalListeners(): void {
    if (this.boundClickHandler) {
      document.removeEventListener('click', this.boundClickHandler);
      this.boundClickHandler = null;
    }
    if (this.boundKeyHandler) {
      document.removeEventListener('keydown', this.boundKeyHandler);
      this.boundKeyHandler = null;
    }
    if (this.boundResizeHandler) {
      window.removeEventListener('resize', this.boundResizeHandler);
      this.boundResizeHandler = null;
    }
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'ghrm-explorer-menus': GhrmExplorerMenus;
  }
}

customElements.define('ghrm-explorer-menus', GhrmExplorerMenus);

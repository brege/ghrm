/**
 * Ambient declarations for ghrm first-party browser modules.
 *
 * Vendor globals are loaded dynamically by ui/src/vendor.ts based on
 * feature detection from data-ghrm-features. These declarations allow
 * TypeScript to check code that references vendor APIs without requiring
 * full type definitions for each library.
 */

// highlight.js
export interface HighlightJS {
  highlightElement(element: Element): void;
}

// mermaid
export interface MermaidAPI {
  initialize(config: Record<string, unknown>): void;
  render(
    id: string,
    code: string,
  ): Promise<{ svg: string; bindFunctions?: (element: Element) => void }>;
  version?: string | (() => string);
}

// svg-pan-zoom
export type SvgPanZoom = (
  svg: SVGElement,
  options?: Record<string, unknown>,
) => SvgPanZoomInstance;
export interface SvgPanZoomInstance {
  zoomIn(): void;
  zoomOut(): void;
  resetZoom(): void;
  center(): void;
  pan(point: { x: number; y: number }): void;
  getPan(): { x: number; y: number };
  resize(): void;
  fit(): void;
  destroy(): void;
}

// KaTeX auto-render
export type RenderMathInElement = (
  element: Element,
  options?: Record<string, unknown>,
) => void;

// Leaflet
export interface LeafletMap {
  setView(center: [number, number], zoom: number): this;
  fitBounds(
    bounds: LeafletLatLngBounds,
    options?: Record<string, unknown>,
  ): this;
  remove(): void;
}
export interface LeafletLatLngBounds {
  isValid(): boolean;
  pad(bufferRatio: number): this;
}
export interface LeafletLayer {
  addTo(map: LeafletMap): this;
  getBounds(): LeafletLatLngBounds;
}
export interface LeafletStatic {
  map(element: Element, options?: Record<string, unknown>): LeafletMap;
  tileLayer(
    urlTemplate: string,
    options?: Record<string, unknown>,
  ): LeafletLayer;
  geoJSON(data: unknown, options?: Record<string, unknown>): LeafletLayer;
  circleMarker(latlng: unknown, options?: Record<string, unknown>): unknown;
}

// topojson
export interface TopoJSONStatic {
  feature(
    topology: unknown,
    object: unknown,
  ): { type: string; features?: unknown[] };
}

// htmx event detail
export interface HtmxEventDetail {
  elt?: Element;
  target?: Element;
  xhr?: XMLHttpRequest;
  [key: string]: unknown;
}

declare global {
  interface Window {
    hljs?: HighlightJS;
    mermaid?: MermaidAPI;
    svgPanZoom?: SvgPanZoom;
    renderMathInElement?: RenderMathInElement;
    L?: LeafletStatic;
    topojson?: TopoJSONStatic;
  }

  interface GlobalEventHandlersEventMap {
    'htmx:beforeBoost': CustomEvent<HtmxEventDetail>;
    'htmx:afterSwap': CustomEvent<HtmxEventDetail>;
    'htmx:historyRestore': CustomEvent<HtmxEventDetail>;
    'htmx:beforeRequest': CustomEvent<HtmxEventDetail>;
    'htmx:afterRequest': CustomEvent<HtmxEventDetail>;
    'htmx:afterSettle': CustomEvent<HtmxEventDetail>;
    'ghrm:contentready': CustomEvent<void>;
    'ghrm:themechange': CustomEvent<{ theme: string }>;
  }
}

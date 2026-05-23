/**
 * Ambient declarations for ghrm first-party browser modules.
 *
 * Vendor globals are loaded dynamically by ui/src/vendor.js based on
 * feature detection from data-ghrm-features. These declarations allow
 * TypeScript to check code that references vendor APIs without requiring
 * full type definitions for each library.
 */

// highlight.js
interface HighlightJS {
  highlightElement(element: Element): void;
}

// mermaid
interface MermaidAPI {
  initialize(config: Record<string, unknown>): void;
  render(
    id: string,
    code: string,
  ): Promise<{ svg: string; bindFunctions?: (element: Element) => void }>;
  version?: string | (() => string);
}

// svg-pan-zoom
type SvgPanZoom = (
  svg: SVGElement,
  options?: Record<string, unknown>,
) => SvgPanZoomInstance;
interface SvgPanZoomInstance {
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
type RenderMathInElement = (
  element: Element,
  options?: Record<string, unknown>,
) => void;

// Leaflet
interface LeafletMap {
  setView(center: [number, number], zoom: number): this;
  fitBounds(
    bounds: LeafletLatLngBounds,
    options?: Record<string, unknown>,
  ): this;
  remove(): void;
}
interface LeafletLatLngBounds {
  isValid(): boolean;
  pad(bufferRatio: number): this;
}
interface LeafletLayer {
  addTo(map: LeafletMap): this;
  getBounds(): LeafletLatLngBounds;
}
interface LeafletStatic {
  map(element: Element, options?: Record<string, unknown>): LeafletMap;
  tileLayer(
    urlTemplate: string,
    options?: Record<string, unknown>,
  ): LeafletLayer;
  geoJSON(data: unknown, options?: Record<string, unknown>): LeafletLayer;
  circleMarker(latlng: unknown, options?: Record<string, unknown>): unknown;
}

// topojson
interface TopoJSONStatic {
  feature(
    topology: unknown,
    object: unknown,
  ): { type: string; features?: unknown[] };
}

// htmx event detail
interface HtmxEventDetail {
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
  }
}

export {};

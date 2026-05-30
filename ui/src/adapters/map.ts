import type { LeafletMap } from '../../types/ghrm';
import { icon } from '../dom';
import { isPrintMode } from '../prefs';
import { hasFeature } from '../vendor';
import { clearError, getSource, setError, themeColors } from './common';

interface MapBlock extends Element {
  _ghrmMap?: LeafletMap | null;
}

interface TopoJSONData {
  objects?: Record<string, unknown>;
}

interface GeoJSONFeature {
  type: string;
  geometry?: { type: string };
  features?: GeoJSONFeature[];
}

function topojsonToGeojson(data: TopoJSONData): GeoJSONFeature {
  const objects = Object.values(data.objects || {});
  const features: GeoJSONFeature[] = [];

  for (const object of objects) {
    const value = window.topojson!.feature(data, object);
    if (value.type === 'FeatureCollection') {
      features.push(...(value.features as GeoJSONFeature[]));
    } else {
      features.push(value as GeoJSONFeature);
    }
  }

  return {
    type: 'FeatureCollection',
    features,
  };
}

function renderMapBlock(block: MapBlock, kind: 'geojson' | 'topojson'): void {
  if (block._ghrmMap) {
    block._ghrmMap.remove();
    block._ghrmMap = null;
  }

  const previous = block.querySelector('.ghrm-map-canvas') as Element;
  const canvas = previous.cloneNode(false) as Element;
  previous.replaceWith(canvas);

  const source = getSource(block);
  if (!source) {
    return;
  }

  const data = JSON.parse(source);
  const geojson = kind === 'topojson' ? topojsonToGeojson(data) : data;
  const colors = themeColors();
  const print = isPrintMode();
  const map = window.L!.map(canvas, {
    attributionControl: false,
    zoomControl: !print,
    scrollWheelZoom: !print,
  });

  if (navigator.onLine) {
    window
      .L!.tileLayer('https://tile.openstreetmap.org/{z}/{x}/{y}.png', {
        maxZoom: 19,
      })
      .addTo(map);
  } else {
    const offline = document.createElement('div');
    offline.className = 'ghrm-map-offline';
    offline.innerHTML = icon('cloud-offline', 'ghrm-action-icon');
    canvas.appendChild(offline);
  }

  const layer = window
    .L!.geoJSON(geojson, {
      style(feature: GeoJSONFeature | undefined) {
        const type = feature?.geometry?.type || '';
        if (type === 'Point' || type === 'MultiPoint') {
          return { color: colors.point, weight: 2 };
        }
        if (type.includes('Line')) {
          return { color: colors.line, weight: 3, opacity: 1 };
        }
        return {
          color: colors.polygon,
          fillColor: colors.polygonFill,
          fillOpacity: 0.3,
          opacity: 0.8,
          weight: 2,
        };
      },
      pointToLayer(_feature: unknown, latlng: unknown) {
        return window.L!.circleMarker(latlng, {
          color: colors.point,
          fillColor: colors.point,
          fillOpacity: 0.9,
          radius: 6,
          weight: 1,
        });
      },
    })
    .addTo(map);

  const bounds = layer.getBounds();
  if (bounds.isValid()) {
    map.fitBounds(bounds.pad(0.1));
  } else {
    map.setView([0, 0], 1);
  }

  block._ghrmMap = map;
}

export async function renderMaps(): Promise<void> {
  if (!hasFeature('map')) return;

  const geojsonBlocks = document.querySelectorAll('.ghrm-geojson');
  const topojsonBlocks = document.querySelectorAll('.ghrm-topojson');
  if (geojsonBlocks.length === 0 && topojsonBlocks.length === 0) return;

  if (!window.L) return;

  for (const block of geojsonBlocks) {
    clearError(block);
    try {
      renderMapBlock(block as MapBlock, 'geojson');
    } catch (error) {
      setError(block, (error as Error).message);
    }
  }
  for (const block of topojsonBlocks) {
    clearError(block);
    try {
      renderMapBlock(block as MapBlock, 'topojson');
    } catch (error) {
      setError(block, (error as Error).message);
    }
  }
}

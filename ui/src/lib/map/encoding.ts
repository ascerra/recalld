// DESIGN §6 visual encoding, as graphology attributes for sigma.
import type { MapTheme } from './theme';
import { alpha } from './theme';
import { nodeSize } from './protocol';

export const EDGE = { parent: 1, associative: 2, causal: 3, contradicts: 4, entity: 5, temporal: 6, supersedes: 7 } as const;
export const EDGE_LABELS: Record<number, string> = {
  1: 'Parent', 2: 'Associative', 3: 'Causal', 4: 'Contradicts', 5: 'Entity', 6: 'Temporal', 7: 'Supersedes',
};

export function nodeAttributes(theme: MapTheme, phase: number, degree: number, permastore: boolean) {
  const size = nodeSize(degree);
  switch (phase) {
    case 1:
      return { size, color: alpha(theme.node, 0.82), ring: alpha(theme.node, 0), visibility: 'visible' as const, permastore };
    case 2:
      return { size, color: alpha(theme.node, 0.38), ring: alpha(theme.node, 0), visibility: 'visible' as const, permastore };
    case 3:
      return { size, color: alpha(theme.node, 0), ring: alpha(theme.node, 0.5), visibility: 'visible' as const, permastore };
    default:
      // Tombstones and memories the decay projection deletes.
      return { size: 1.6, color: alpha(theme.node, 0.35), ring: alpha(theme.node, 0), visibility: 'hidden' as const, permastore: false };
  }
}

export function edgeAttributes(theme: MapTheme, type: number, weight: number) {
  const a = 0.4 * (0.55 + 0.6 * weight);
  const base = type === EDGE.contradicts ? theme.contradicts : theme.edge;
  const stronger = type === EDGE.supersedes ? 1.25 : 1;
  const dashed = type === EDGE.entity ? [1.5, 2.5] : type === EDGE.contradicts ? [3, 3] : null;
  const color = alpha(base, Math.min(1, a * stronger));
  return {
    size: type === EDGE.supersedes ? 1 : 0.8,
    color: dashed ? alpha(base, 0) : color,
    dashColor: dashed ? color : alpha(base, 0),
    dashSize: dashed ? dashed[0] : 0,
    gapSize: dashed ? dashed[1] : 0,
    baseColor: color,
  };
}

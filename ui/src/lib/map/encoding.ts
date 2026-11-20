// DESIGN §6 visual encoding, as graphology attributes for sigma.
import type { MapTheme } from './theme';
import { alpha } from './theme';
import { displaySize } from './protocol';

export const EDGE = { parent: 1, associative: 2, causal: 3, contradicts: 4, entity: 5, temporal: 6, supersedes: 7 } as const;
export const EDGE_LABELS: Record<number, string> = {
  1: 'Parent', 2: 'Associative', 3: 'Causal', 4: 'Contradicts', 5: 'Entity', 6: 'Temporal', 7: 'Supersedes',
};

/** `strength`: retrievability now (0..1); a memory just stored is 1.
 *  `tone`: the memory's own color (its namespace's tone, varied slightly). */
export function nodeAttributes(theme: MapTheme, phase: number, degree: number, permastore: boolean, strength = 1, tone = theme.node) {
  const size = displaySize(degree, strength);
  const node = tone;
  switch (phase) {
    case 1:
      return { size, color: alpha(node, 0.82), ring: alpha(node, 0), visibility: 'visible' as const, permastore };
    case 2:
      return { size, color: alpha(node, 0.38), ring: alpha(node, 0), visibility: 'visible' as const, permastore };
    case 3:
      return { size, color: alpha(node, 0), ring: alpha(node, 0.5), visibility: 'visible' as const, permastore };
    default:
      // Tombstones and memories the decay projection deletes.
      return { size: 1.6, color: alpha(node, 0.35), ring: alpha(node, 0), visibility: 'hidden' as const, permastore: false };
  }
}

export function edgeAttributes(theme: MapTheme, type: number, weight: number) {
  // Soft solid lines whose opacity carries the weight, so weak links recede
  // and dense clusters read as a haze rather than a scratchy mesh. Only
  // contradictions are dashed: fine dashes in a dense cluster look like noise.
  const a = 0.32 * (0.3 + 0.7 * weight);
  const base = type === EDGE.contradicts ? theme.contradicts : theme.edge;
  const stronger = type === EDGE.supersedes ? 1.25 : 1;
  const dashed = type === EDGE.contradicts ? [3, 3] : null;
  const color = alpha(base, Math.min(1, a * stronger));
  return {
    size: type === EDGE.supersedes ? 1.2 : 1,
    color: dashed ? alpha(base, 0) : color,
    dashColor: dashed ? color : alpha(base, 0),
    dashSize: dashed ? dashed[0] : 0,
    gapSize: dashed ? dashed[1] : 0,
    baseColor: color,
  };
}

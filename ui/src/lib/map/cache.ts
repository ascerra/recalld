// Map positions cached per memory in IndexedDB (the only thing the UI
// stores: no memory content). One entry per scope: ids → x, y.
import { get, set } from 'idb-keyval';

// Bump the version when the layout forces change, so old layouts are redone.
// Each spacing keeps its own layout; the default (1) keeps the original key.
const key = (scope: string | null, spacing: number) =>
  `positions:v7:${scope ?? '*'}${spacing === 1 ? '' : `:s${spacing}`}`;

export async function loadPositions(scope: string | null, spacing = 1): Promise<Map<string, [number, number]>> {
  try {
    const stored = (await get(key(scope, spacing))) as { ids: string[]; xy: Float32Array } | undefined;
    const out = new Map<string, [number, number]>();
    if (stored) stored.ids.forEach((id, i) => out.set(id, [stored.xy[2 * i], stored.xy[2 * i + 1]]));
    return out;
  } catch {
    return new Map();
  }
}

export async function savePositions(scope: string | null, ids: string[], x: Float32Array, y: Float32Array, spacing = 1) {
  const xy = new Float32Array(ids.length * 2);
  for (let i = 0; i < ids.length; i++) {
    xy[2 * i] = x[i];
    xy[2 * i + 1] = y[i];
  }
  try {
    await set(key(scope, spacing), { ids, xy });
  } catch {
    /* storage unavailable: the next visit lays out again */
  }
}

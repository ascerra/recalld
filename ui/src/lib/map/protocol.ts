// Messages between the map and its layout worker.
export type ToWorker =
  | {
      type: 'init';
      n: number;
      src: Uint32Array;
      dst: Uint32Array;
      weight: Float32Array;
      degree: Uint32Array;
      x: Float32Array;
      y: Float32Array;
      /** 1 where the position came from the cache. */
      known: Uint8Array;
      maxDegree: number;
    }
  | { type: 'drag'; phase: 'start' | 'move' | 'end'; index: number; x: number; y: number }
  | { type: 'stop' };

export type FromWorker =
  | { type: 'tick'; x: Float32Array; y: Float32Array; alpha: number }
  | { type: 'settled'; x: Float32Array; y: Float32Array };

let maxDegree = 1;
/** Size nodes relative to this graph's most-connected memory. */
export function setDegreeScale(max: number) {
  maxDegree = Math.max(1, max);
}
/** 0 for an unlinked memory, 1 for the most-connected one (log-scaled). */
export const hubness = (degree: number) => Math.log(1 + degree) / Math.log(1 + maxDegree);
/** Node radius at zoom 1: grows with links, scaled to the graph's own range
 *  so hubs stand out (DESIGN §6 used 2.4 + 1.9·ln(1+degree); widened on review). */
export const nodeSize = (degree: number) => 2.5 + 9.5 * Math.pow(Math.min(1, hubness(degree)), 1.6);

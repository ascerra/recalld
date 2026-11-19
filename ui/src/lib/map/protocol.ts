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
    }
  | { type: 'drag'; phase: 'start' | 'move' | 'end'; index: number; x: number; y: number }
  | { type: 'stop' };

export type FromWorker =
  | { type: 'tick'; x: Float32Array; y: Float32Array; alpha: number }
  | { type: 'settled'; x: Float32Array; y: Float32Array };

/** Node radius at zoom 1 (DESIGN §6). */
export const nodeSize = (degree: number) => 2.4 + 1.9 * Math.log(1 + degree);

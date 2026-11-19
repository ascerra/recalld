// Decodes graph_snapshot's base64 little-endian columns into typed arrays.
import type { GraphSnapshot, NamespaceRef } from './types';
import { get } from './client';

function bytes(b64: string): Uint8Array<ArrayBuffer> {
  const bin = atob(b64);
  const out = new Uint8Array(new ArrayBuffer(bin.length));
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}
const u32 = (s: string) => new Uint32Array(bytes(s).buffer);
const f32 = (s: string) => new Float32Array(bytes(s).buffer);
const f64 = (s: string) => new Float64Array(bytes(s).buffer);

export interface Graph {
  generatedAt: number;
  namespaces: NamespaceRef[];
  ids: string[];
  index: Map<string, number>;
  ns: Uint32Array;
  phase: Uint8Array;
  strength: Float32Array;
  decayStrength: Float32Array;
  stability: Float32Array;
  createdAt: Float64Array;
  lastAccess: Float64Array;
  degree: Uint32Array;
  permastore: Uint8Array;
  src: Uint32Array;
  dst: Uint32Array;
  type: Uint8Array;
  weight: Float32Array;
}

export function decode(first: GraphSnapshot, pages: GraphSnapshot[] = []): Graph {
  const n = first.nodes;
  if (!n) throw new Error('first snapshot page has no nodes');
  const concat = <T extends Uint32Array | Uint8Array | Float32Array>(
    make: (s: string) => T,
    pick: (p: GraphSnapshot) => string,
    Ctor: { new (len: number): T },
  ): T => {
    const parts = [first, ...pages].map((p) => make(pick(p)));
    if (parts.length === 1) return parts[0];
    const out = new Ctor(parts.reduce((a, p) => a + p.length, 0));
    let o = 0;
    for (const p of parts) {
      out.set(p as never, o);
      o += p.length;
    }
    return out;
  };
  return {
    generatedAt: first.generatedAt,
    namespaces: first.namespaces,
    ids: n.ids,
    index: new Map(n.ids.map((id, i) => [id, i])),
    ns: u32(n.ns),
    phase: bytes(n.phase),
    strength: f32(n.strength),
    decayStrength: f32(n.decayStrength),
    stability: f32(n.stability),
    createdAt: f64(n.createdAt),
    lastAccess: f64(n.lastAccess),
    degree: u32(n.degree),
    permastore: bytes(n.permastore),
    src: concat(u32, (p) => p.edges.src, Uint32Array),
    dst: concat(u32, (p) => p.edges.dst, Uint32Array),
    type: concat(bytes, (p) => p.edges.type, Uint8Array),
    weight: concat(f32, (p) => p.edges.weight, Float32Array),
  };
}

/** Fetch every page of the snapshot for `namespace` (null = all). */
export async function fetchGraph(namespace: string | null): Promise<Graph> {
  const first = await get<GraphSnapshot>('/graph', { namespace });
  const pages: GraphSnapshot[] = [];
  let next = first.nextEdgeOffset;
  while (next !== null) {
    const page = await get<GraphSnapshot>('/graph', { namespace, edgeOffset: next });
    if (page.nodeCount !== first.nodeCount) return fetchGraph(namespace); // graph changed mid-read
    pages.push(page);
    next = page.nextEdgeOffset;
  }
  return decode(first, pages);
}

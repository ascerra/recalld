// The map's physics, off the main thread (DESIGN §6): d3-force with
// Barnes–Hut many-body, link, collide and gentle centering. Positions are
// streamed back as transferable Float32Arrays.
import { forceCollide, forceLink, forceManyBody, forceSimulation, forceX, forceY, type SimulationNodeDatum } from 'd3-force';
import { hubness, nodeSize, setDegreeScale, type FromWorker, type ToWorker } from './protocol';

interface Node extends SimulationNodeDatum {
  i: number;
  r: number;
  /** 0..1, how connected this memory is relative to the graph. */
  hub: number;
}

let sim: ReturnType<typeof forceSimulation<Node>> | null = null;
let nodes: Node[] = [];
let adjacency: number[][] = [];
let running = false;
let pinned: number[] = [];
// The offset that keeps the layout centered. Recomputed only until the first
// settle; afterwards fixed, so dragging one node never shifts the rest.
let offset: [number, number] | null = null;

const post = (m: FromWorker, transfer: Transferable[]) => (self as unknown as Worker).postMessage(m, transfer);

function positions() {
  const n = nodes.length;
  const x = new Float32Array(n), y = new Float32Array(n);
  let cx = 0, cy = 0;
  if (offset) {
    [cx, cy] = offset;
  } else {
    for (const d of nodes) {
      cx += d.x!;
      cy += d.y!;
    }
    cx /= n || 1;
    cy /= n || 1;
  }
  for (let k = 0; k < n; k++) {
    x[k] = nodes[k].x! - cx;
    y[k] = nodes[k].y! - cy;
  }
  return { x, y };
}

async function run() {
  if (running || !sim) return;
  running = true;
  let frames = 0;
  while (sim && sim.alpha() > sim.alphaMin()) {
    sim.tick(2);
    if (++frames % 2 === 0) {
      const { x, y } = positions();
      post({ type: 'tick', x, y, alpha: sim.alpha() }, [x.buffer, y.buffer]);
    }
    // Yield so drag messages arrive between ticks.
    await new Promise((r) => setTimeout(r, 0));
  }
  running = false;
  if (sim) {
    const { x, y } = positions();
    freezeOffset();
    post({ type: 'settled', x, y }, [x.buffer, y.buffer]);
  }
}

function freezeOffset() {
  if (offset) return;
  let cx = 0, cy = 0;
  for (const d of nodes) {
    cx += d.x!;
    cy += d.y!;
  }
  offset = [cx / (nodes.length || 1), cy / (nodes.length || 1)];
}

self.onmessage = (e: MessageEvent<ToWorker>) => {
  const m = e.data;
  if (m.type === 'init') {
    sim?.stop();
    offset = null;
    setDegreeScale(m.maxDegree);
    const unknown: number[] = [];
    adjacency = Array.from({ length: m.n }, () => []);
    for (let k = 0; k < m.src.length; k++) {
      adjacency[m.src[k]].push(m.dst[k]);
      adjacency[m.dst[k]].push(m.src[k]);
    }
    nodes = Array.from({ length: m.n }, (_, i) => {
      if (!m.known[i]) unknown.push(i);
      return { i, r: nodeSize(m.degree[i]), hub: hubness(m.degree[i]), x: m.x[i], y: m.y[i] };
    });
    // New memories appear next to their neighbors (DESIGN §6).
    const anyKnown = unknown.length < m.n;
    for (const i of unknown) {
      const near = adjacency[i].filter((j) => m.known[j]);
      if (anyKnown && near.length) {
        nodes[i].x = near.reduce((a, j) => a + nodes[j].x!, 0) / near.length + (Math.random() - 0.5) * 8;
        nodes[i].y = near.reduce((a, j) => a + nodes[j].y!, 0) / near.length + (Math.random() - 0.5) * 8;
      } else if (anyKnown) {
        nodes[i].x = (Math.random() - 0.5) * 50;
        nodes[i].y = (Math.random() - 0.5) * 50;
      } else {
        nodes[i].x = undefined;
        nodes[i].y = undefined;
      }
    }
    const links = Array.from(m.src, (s, k) => ({ source: s, target: m.dst[k], w: m.weight[k] }));
    sim = forceSimulation(nodes)
      // Room to read: strong repulsion, long weak links, generous spacing.
      // Hubs push harder for room and pull harder toward the center, so they
      // anchor their clusters and sparse memories drift outward.
      .force('charge', forceManyBody<Node>().strength((d) => -40 - 90 * d.hub).theta(0.9).distanceMax(900))
      .force('link', forceLink<Node, { source: number; target: number; w: number }>(links).id((d) => d.i).distance(48).strength((l) => 0.03 + 0.12 * l.w))
      .force('collide', forceCollide<Node>((d) => d.r * 2 + 4).iterations(1))
      .force('x', forceX<Node>(0).strength((d) => 0.008 + 0.04 * d.hub))
      .force('y', forceY<Node>(0).strength((d) => 0.008 + 0.04 * d.hub))
      .stop();
    // Cached layouts open settled: barely reheat; a fully new one starts hot.
    sim.alpha(unknown.length === 0 ? 0 : anyKnown ? Math.min(0.3, 0.05 + unknown.length / m.n) : 1);
    if (unknown.length === 0) {
      offset = [0, 0]; // cached layouts are already centered
      const { x, y } = positions();
      post({ type: 'settled', x, y }, [x.buffer, y.buffer]);
    } else run();
  } else if (m.type === 'drag' && sim) {
    const n = nodes[m.index];
    if (m.phase === 'start') {
      // Reheat only the dragged node's 2-hop neighborhood: pin everything else.
      const near = new Set([m.index]);
      for (const j of adjacency[m.index]) {
        near.add(j);
        for (const k of adjacency[j]) near.add(k);
      }
      pinned = [];
      for (const other of nodes)
        if (!near.has(other.i)) {
          other.fx = other.x;
          other.fy = other.y;
          pinned.push(other.i);
        }
      // A gentle, well-damped reheat of the neighborhood only.
      freezeOffset();
      sim.velocityDecay(0.6).alphaTarget(0.04).alpha(Math.max(sim.alpha(), 0.06));
    }
    // The main thread works in centered coordinates; the worker in raw ones.
    n.fx = m.x + offset![0];
    n.fy = m.y + offset![1];
    if (m.phase === 'end') {
      n.fx = n.fy = null;
      sim.alphaTarget(0);
      sim.velocityDecay(0.4);
      // Release the pinned nodes once the neighborhood has settled.
      const release = pinned;
      pinned = [];
      setTimeout(() => {
        for (const i of release) nodes[i].fx = nodes[i].fy = null;
      }, 1500);
    }
    run();
  } else if (m.type === 'stop') {
    sim?.stop();
    sim = null;
  }
};

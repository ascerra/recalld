// The map's physics, off the main thread (DESIGN §6): d3-force with
// Barnes–Hut many-body, link, collide and gentle centering. Positions are
// streamed back as transferable Float32Arrays.
import { forceCollide, forceLink, forceManyBody, forceSimulation, forceX, forceY, type SimulationNodeDatum } from 'd3-force';
import { nodeSize, type FromWorker, type ToWorker } from './protocol';

interface Node extends SimulationNodeDatum {
  i: number;
  r: number;
}

let sim: ReturnType<typeof forceSimulation<Node>> | null = null;
let nodes: Node[] = [];
let adjacency: number[][] = [];
let running = false;
let pinned: number[] = [];

const post = (m: FromWorker, transfer: Transferable[]) => (self as unknown as Worker).postMessage(m, transfer);

function positions() {
  const n = nodes.length;
  const x = new Float32Array(n), y = new Float32Array(n);
  // Keep the layout centered on the origin so the camera frame stays put.
  let cx = 0, cy = 0;
  for (const d of nodes) {
    cx += d.x!;
    cy += d.y!;
  }
  cx /= n || 1;
  cy /= n || 1;
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
    post({ type: 'settled', x, y }, [x.buffer, y.buffer]);
  }
}

self.onmessage = (e: MessageEvent<ToWorker>) => {
  const m = e.data;
  if (m.type === 'init') {
    sim?.stop();
    const unknown: number[] = [];
    adjacency = Array.from({ length: m.n }, () => []);
    for (let k = 0; k < m.src.length; k++) {
      adjacency[m.src[k]].push(m.dst[k]);
      adjacency[m.dst[k]].push(m.src[k]);
    }
    nodes = Array.from({ length: m.n }, (_, i) => {
      if (!m.known[i]) unknown.push(i);
      return { i, r: nodeSize(m.degree[i]), x: m.x[i], y: m.y[i] };
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
      .force('charge', forceManyBody<Node>().strength(-18).theta(0.9).distanceMax(400))
      .force('link', forceLink<Node, { source: number; target: number; w: number }>(links).id((d) => d.i).distance(24).strength((l) => 0.08 + 0.3 * l.w))
      .force('collide', forceCollide<Node>((d) => d.r + 1.5).iterations(1))
      .force('x', forceX(0).strength(0.015))
      .force('y', forceY(0).strength(0.015))
      .stop();
    // Cached layouts open settled: barely reheat; a fully new one starts hot.
    sim.alpha(unknown.length === 0 ? 0 : anyKnown ? Math.min(0.3, 0.05 + unknown.length / m.n) : 1);
    if (unknown.length === 0) {
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
      sim.alphaTarget(0.15).alpha(Math.max(sim.alpha(), 0.15));
    }
    n.fx = m.x;
    n.fy = m.y;
    if (m.phase === 'end') {
      n.fx = n.fy = null;
      sim.alphaTarget(0);
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

<!-- The Memory map canvas (DESIGN §6): sigma on WebGL2 for nodes and edges,
     d3-force in a worker, rings and collision-free labels on a 2D overlay
     redrawn only when sigma renders. Nothing animates while idle. -->
<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import Graph from 'graphology';
  import Sigma from 'sigma';
  import { layerDashed, layerPlain, pathLine, sdfCircle } from 'sigma/rendering';
  import { layerBorder } from '@sigma/node-border';
  import type { Graph as Snapshot } from '$lib/api/snapshot';
  import { memoryRows } from '$lib/api/memories';
  import { placeLabels, PointGrid, type LabelCandidate } from '$lib/labels';
  import { ago, phaseName, truncateWords, duration } from '$lib/format';
  import { DAY_MS, phaseFor, retrievability } from '$lib/fsrs';
  import { readTheme, type MapTheme } from './theme';
  import { edgeAttributes, nodeAttributes, EDGE } from './encoding';
  import { loadPositions, savePositions } from './cache';
  import { setDegreeScale, type FromWorker, type ToWorker } from './protocol';
  import LayoutWorker from './layout.worker.ts?worker';
  import { LightLayer, affine } from './light';
  import { choreograph } from './choreo';
  import { rgb, alpha } from './theme';
  import { live, type Received } from '$lib/live.svelte';

  let {
    snapshot,
    scopeKey,
    selectedId = null,
    projectionDays = 0,
    phases,
    edgeTypes,
    highlighted = [],
    onselect,
    onready,
    onsettled,
    onfocuschange,
  }: {
    snapshot: Snapshot;
    scopeKey: string | null;
    selectedId?: string | null;
    projectionDays?: number;
    phases: Set<number>;
    edgeTypes: Set<number>;
    /** Ids labeled by default (the current or most recent recall's results). */
    highlighted?: string[];
    onselect: (id: string | null) => void;
    onready?: (info: { cached: boolean; ms: number }) => void;
    /** The layout stopped moving (immediately for a cached layout). */
    onsettled?: () => void;
    /** The local graph (a memory's neighborhood) was entered or left. */
    onfocuschange?: (focused: boolean) => void;
  } = $props();

  let container: HTMLDivElement;
  let overlay: HTMLCanvasElement;
  let lightCanvas: HTMLCanvasElement;
  let light: LightLayer | null = null;
  // Live state: result rings and suppressed nodes expire; labels follow the
  // current or most recent recall.
  let rings: { id: string; until: number; linked: boolean }[] = [];
  let liveLabels: string[] = [];
  const reducedMotion = typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;
  let tooltip = $state<{ id: string; x: number; y: number; flip: boolean } | null>(null);
  let summaries = $state<Map<string, string | null>>(new Map());
  let sigma: Sigma | null = null;
  let worker: Worker | null = null;
  let graph: Graph | null = null;
  let theme: MapTheme;
  let hovered: string | null = null;
  let focus: Set<string> | null = $state(null);
  let hoverTimer: ReturnType<typeof setTimeout> | undefined;
  let visibleIds = $state<string[]>([]);
  const started = performance.now();
  const perf = window as unknown as {
    __recalldSettled?: boolean;
    __recalldCached?: boolean;
    __recalldLit?: number[];
    __recalldLitPair?: [number, number];
    __recalldNodeAt?: () => { x: number; y: number } | null;
  };
  perf.__recalldSettled = false;
  // Test hook: viewport positions of every visible node.
  (perf as { __recalldPositions?: () => { id: string; x: number; y: number; size: number }[] }).__recalldPositions = () => {
    const out: { id: string; x: number; y: number; size: number }[] = [];
    graph?.forEachNode((id, a) => {
      if (a.visibility === 'visible') out.push({ id, size: a.size as number, ...sigma!.graphToViewport(a as { x: number; y: number }) });
    });
    return out;
  };
  perf.__recalldNodeAt = () => {
    if (!graph || !sigma) return null;
    let best: { x: number; y: number } | null = null;
    let bestD = Infinity;
    const w = container.clientWidth / 2, h = container.clientHeight / 2;
    graph.forEachNode((_, a) => {
      if (a.visibility !== 'visible') return;
      const p = sigma!.graphToViewport(a as { x: number; y: number });
      const d = Math.hypot(p.x - w, p.y - h);
      if (d < bestD) {
        bestD = d;
        best = p;
      }
    });
    return best;
  };

  // ── Graph construction ────────────────────────────────────────────────
  function build(s: Snapshot, cached: Map<string, [number, number]>) {
    const g = new Graph({ type: 'directed', multi: true, allowSelfLoops: false });
    const n = s.ids.length;
    let maxDegree = 1;
    for (let i = 0; i < n; i++) maxDegree = Math.max(maxDegree, s.degree[i]);
    setDegreeScale(maxDegree);
    const x = new Float32Array(n), y = new Float32Array(n), known = new Uint8Array(n);
    for (let i = 0; i < n; i++) {
      const c = cached.get(s.ids[i]);
      if (c) {
        x[i] = c[0];
        y[i] = c[1];
        known[i] = 1;
      } else {
        // Phyllotaxis seed: deterministic and spread out.
        const r = 10 * Math.sqrt(i + 0.5), a = i * 2.399963;
        x[i] = r * Math.cos(a);
        y[i] = r * Math.sin(a);
      }
      g.addNode(s.ids[i], {
        x: x[i],
        y: y[i],
        index: i,
        ...nodeAttributes(theme, s.phase[i], s.degree[i], s.permastore[i] === 1),
      });
    }
    for (let k = 0; k < s.src.length; k++) {
      const a = edgeAttributes(theme, s.type[k], s.weight[k]);
      g.addEdgeWithKey(`e${k}`, s.ids[s.src[k]], s.ids[s.dst[k]], {
        ...a,
        type: s.type[k],
        visibility: edgeTypes.has(s.type[k]) ? 'visible' : 'hidden',
      });
    }
    return { g, x, y, known };
  }

  // ── Encoding updates (phase filter, decay projection, links menu) ─────
  function projectedPhase(i: number, at: number): number {
    const s = snapshot;
    if (s.permastore[i] || s.phase[i] === 4 || s.stability[i] <= 0) return s.phase[i];
    const ns = s.namespaces.find((n) => n.id === s.ns[i]);
    const r = retrievability(Math.max(0, at - s.lastAccess[i]) / DAY_MS, s.stability[i], 1);
    // The projection never makes a memory stronger than recalld says it is now.
    return Math.max(s.phase[i], phaseFor(r, (ns?.thresholds as [number, number, number]) ?? undefined));
  }

  function reencode() {
    if (!graph) return;
    const at = Date.now() + projectionDays * DAY_MS;
    graph.forEachNode((id, attrs) => {
      const i = attrs.index as number;
      const phase = projectionDays > 0 ? projectedPhase(i, at) : snapshot.phase[i];
      const enc = nodeAttributes(theme, phase, snapshot.degree[i], snapshot.permastore[i] === 1);
      const hidden = !phases.has(phase) || (focus !== null && !focus.has(id));
      graph!.mergeNodeAttributes(id, { ...enc, phase, visibility: hidden ? 'hidden' : enc.visibility });
    });
    graph.forEachEdge((id, attrs, src, dst) => {
      const show =
        edgeTypes.has(attrs.type as number) &&
        graph!.getNodeAttribute(src, 'visibility') === 'visible' &&
        graph!.getNodeAttribute(dst, 'visibility') === 'visible';
      graph!.setEdgeAttribute(id, 'visibility', show ? 'visible' : 'hidden');
    });
  }

  $effect(() => {
    // Re-run when any of these change.
    projectionDays;
    phases;
    edgeTypes;
    focus;
    reencode();
    sigma?.refresh();
  });

  $effect(() => {
    selectedId;
    highlighted;
    sigma?.refresh({ skipIndexation: true });
    if (selectedId) ensureSummaries([selectedId]);
    ensureSummaries(highlighted);
  });

  // ── Summaries for labels and tooltips (read-only lookups) ─────────────
  let pendingIds = new Set<string>();
  let fetchTimer: ReturnType<typeof setTimeout> | undefined;
  function ensureSummaries(ids: string[]) {
    for (const id of ids) if (!summaries.has(id)) pendingIds.add(id);
    if (!pendingIds.size) return;
    clearTimeout(fetchTimer);
    fetchTimer = setTimeout(async () => {
      const batch = [...pendingIds].slice(0, 200);
      batch.forEach((id) => pendingIds.delete(id));
      const rows = await memoryRows(batch).catch(() => []);
      const next = new Map(summaries);
      for (const id of batch) next.set(id, rows.find((r) => r.id === id)?.summary ?? null);
      summaries = next;
      sigma?.refresh({ skipIndexation: true });
      if (pendingIds.size) ensureSummaries([]);
    }, 30);
  }

  // ── Overlay: rings and labels, drawn after each sigma render ──────────
  function drawOverlay() {
    if (!sigma || !graph) return;
    const ctx = overlay.getContext('2d')!;
    const dpr = window.devicePixelRatio || 1;
    const w = container.clientWidth, h = container.clientHeight;
    if (overlay.width !== w * dpr || overlay.height !== h * dpr) {
      overlay.width = w * dpr;
      overlay.height = h * dpr;
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    const ratio = sigma.getCamera().getState().ratio;
    const scale = 1 / Math.sqrt(ratio);
    const vp = (id: string) => sigma!.graphToViewport(graph!.getNodeAttributes(id) as { x: number; y: number });
    const radius = (id: string) => (graph!.getNodeAttribute(id, 'size') as number) * scale;
    const ring = (id: string, extra: number, color: string, width: number) => {
      if (!graph!.hasNode(id) || graph!.getNodeAttribute(id, 'visibility') !== 'visible') return;
      const p = vp(id);
      ctx.beginPath();
      ctx.arc(p.x, p.y, radius(id) + extra, 0, Math.PI * 2);
      ctx.strokeStyle = color;
      ctx.lineWidth = width;
      ctx.stroke();
    };

    // Permastore halos (only when zoomed in enough to see them).
    const inView: string[] = [];
    const xs: number[] = [], ys: number[] = [];
    graph.forEachNode((id, a) => {
      if (a.visibility !== 'visible') return;
      const p = vp(id);
      if (p.x < -20 || p.y < -20 || p.x > w + 20 || p.y > h + 20) return;
      inView.push(id);
      xs.push(p.x);
      ys.push(p.y);
      if (a.permastore) ring(id, 3.2, theme.permastore, 1);
    });
    for (const id of highlighted) ring(id, 4.5, theme.accent, 1.3);
    const t = performance.now();
    rings = rings.filter((r) => r.until > t);
    for (const r of rings) ring(r.id, 4.5, r.linked ? theme.moss : theme.accent, 1.3);
    // Links growing out of a new memory.
    for (const g of growing) {
      const u = Math.min(1, Math.max(0, (t - g.start) / g.dur));
      const k = g.retract ? (t < g.start ? 1 : 1 - easeIn(u)) : easeOut(u);
      if (k <= 0 || !graph.hasNode(g.from) || !graph.hasNode(g.to)) continue;
      const a = vp(g.from), b = vp(g.to);
      ctx.beginPath();
      ctx.moveTo(a.x, a.y);
      ctx.lineTo(a.x + (b.x - a.x) * k, a.y + (b.y - a.y) * k);
      ctx.strokeStyle = alpha(theme.edge, 0.7);
      ctx.lineWidth = 1;
      ctx.stroke();
    }
    if (hovered) ring(hovered, 5, theme.text, 1.5);
    if (selectedId) ring(selectedId, 7, theme.text, 1.6);

    // Labels: seeds/results, selection, hover; more as you zoom in.
    const order = [...new Set([...liveLabels, ...highlighted])];
    if (selectedId && !order.includes(selectedId)) order.push(selectedId);
    if (hovered && !order.includes(hovered)) order.push(hovered);
    if (ratio < 0.6) {
      const extra = inView
        .filter((id) => !order.includes(id))
        .sort((a, b) => (graph!.getNodeAttribute(b, 'size') as number) - (graph!.getNodeAttribute(a, 'size') as number))
        .slice(0, Math.round(12 / ratio));
      ensureSummaries(extra.slice(0, 200));
      order.push(...extra);
    }
    ctx.font = `11.5px ${theme.fontSans}`;
    const cands: LabelCandidate[] = [];
    for (const id of order) {
      if (!graph.hasNode(id) || graph.getNodeAttribute(id, 'visibility') !== 'visible') continue;
      const text = summaries.get(id);
      if (text === undefined) continue;
      const label = text === null ? 'Summary dropped' : truncateWords(text, 26);
      const p = vp(id);
      cands.push({ id, x: p.x, y: p.y, r: radius(id) + 2, width: ctx.measureText(label).width + 4, height: 15 });
    }
    const grid = new PointGrid(xs, ys, 24);
    ctx.textBaseline = 'top';
    ctx.lineJoin = 'round';
    for (const pl of placeLabels(cands, { width: w, height: h }, grid, 4)) {
      const text = summaries.get(pl.id);
      const label = text === null || text === undefined ? 'Summary dropped' : truncateWords(text, 26);
      const strong = liveLabels.includes(pl.id) || highlighted.includes(pl.id) || pl.id === selectedId;
      ctx.font = `${strong ? 600 : 400} 11.5px ${theme.fontSans}`;
      ctx.lineWidth = 4;
      ctx.strokeStyle = theme.halo;
      ctx.strokeText(label, pl.x + 2, pl.y + 1);
      ctx.fillStyle = theme.text;
      ctx.fillText(label, pl.x + 2, pl.y + 1);
    }
    // Hovered and selected are often the same node: keep each id once.
    visibleIds = [...new Set(order)].filter((id) => summaries.has(id)).slice(0, 100);
  }

  // ── Live activity → light and incremental graph updates ───────────────
  function position(id: string) {
    if (!graph?.hasNode(id) || graph.getNodeAttribute(id, 'visibility') !== 'visible') return null;
    return graph.getNodeAttributes(id) as { x: number; y: number };
  }

  function onLive(r: Received, replay?: { speed: number }) {
    if (!graph || !sigma || !light) return;
    const e = r.event;
    if (!replay) applyToGraph(e);
    const slot = 'agent' in e ? live.agentSlot(e.agent) : 0;
    const colors = { agent: rgb(theme.agents[slot]), lit: rgb(theme.nodeLit), moss: rgb(theme.moss), cool: rgb(theme.suppressed) };
    const scale = 1 / Math.sqrt(sigma.getCamera().getState().ratio);
    const c = choreograph(e, position, performance.now(), colors, { speed: replay?.speed ?? 1, reducedMotion, scale });
    c.pulses.forEach((p) => light!.pulse(p));
    c.glows.forEach((g) => light!.glow(g));
    if (e.kind === 'recall') {
      rings = [...rings.filter((x) => !c.rings.some((y) => y.id === x.id)), ...c.rings];
      liveLabels = c.labels.slice(0, 12);
      ensureSummaries(liveLabels);
      // Flash each recalled memory when the light reaches it, then return it
      // to its own phase shading. Suppression shows only as a brief cool glow.
      const t0 = performance.now();
      for (const f of c.flashes) {
        if (!graph.hasNode(f.id)) continue;
        setTimeout(() => flash(f.id), Math.max(0, f.at - t0));
      }
      const last = Math.max(0, ...c.rings.map((x) => x.until)) - performance.now();
      setTimeout(() => sigma?.refresh({ skipIndexation: true }), last + 50);
    }
    light.wake();
    sigma.refresh({ skipIndexation: true });
    if (e.kind === 'recall' && !replay && e.seq < 1_000_000) {
      requestAnimationFrame(() => {
        perf.__recalldLitPair = [Date.now(), e.at];
        (perf.__recalldLit ??= []).push(Date.now());
      });
    }
  }

  const flashUntil = new Map<string, number>();
  const FLASH_MS = 1500;
  function flash(id: string) {
    if (!graph?.hasNode(id)) return;
    flashUntil.set(id, performance.now() + FLASH_MS);
    graph.setNodeAttribute(id, 'color', alpha(theme.nodeLit, 0.95));
    sigma?.refresh({ skipIndexation: true });
    setTimeout(() => {
      if (!graph?.hasNode(id) || (flashUntil.get(id) ?? 0) > performance.now()) return;
      flashUntil.delete(id);
      const a = graph.getNodeAttributes(id);
      const i = a.index as number;
      const enc = nodeAttributes(theme, (a.phase as number) ?? 1, i >= 0 ? snapshot.degree[i] : graph.degree(id), i >= 0 && snapshot.permastore[i] === 1);
      graph.setNodeAttribute(id, 'color', enc.color);
      sigma?.refresh({ skipIndexation: true });
    }, FLASH_MS + 20);
  }

  // ── A new memory's arrival ────────────────────────────────────────────
  // It pops in with a slight overshoot and drifts into place, nearby memories
  // are nudged outward and ease back, and its links grow out of it.
  interface Motion { x0: number; y0: number; x1: number; y1: number; start: number; dur: number; ripple: boolean; size?: number; shrink?: boolean; done?: () => void }
  const motions = new Map<string, Motion>();
  let growing: { from: string; to: string; start: number; dur: number; key: string; visible: boolean; retract?: boolean }[] = [];
  let animRaf = 0;
  let lastTick = 0;
  /** When each live memory appeared, so its first links can grow out of it. */
  const bornAt = new Map<string, number>();
  /** Graph units per screen pixel, so motion looks the same at any zoom. */
  const unitsPerPx = () => {
    const a = sigma!.viewportToGraph({ x: 0, y: 0 }), b = sigma!.viewportToGraph({ x: 1, y: 0 });
    return Math.hypot(b.x - a.x, b.y - a.y);
  };
  const easeOut = (t: number) => 1 - Math.pow(1 - t, 3);
  const easeBack = (t: number) => 1 + 2.7 * Math.pow(t - 1, 3) + 1.7 * Math.pow(t - 1, 2);
  const easeIn = (t: number) => t * t * t;

  function animate() {
    animRaf = 0;
    if (!graph) return;
    const now = performance.now();
    for (const [id, m] of motions) {
      if (!graph.hasNode(id)) {
        motions.delete(id);
        continue;
      }
      const t = Math.min(1, (now - m.start) / m.dur);
      if (t < 0) continue;
      // A ripple goes out and comes back; a move eases to its target.
      const k = m.ripple ? (t >= 1 ? 0 : Math.sin(Math.PI * t)) : easeOut(t);
      const a: Record<string, number> = { x: m.x0 + (m.x1 - m.x0) * k, y: m.y0 + (m.y1 - m.y0) * k };
      if (m.size !== undefined) a.size = m.shrink ? m.size * (1 - easeIn(t)) : m.size * (t >= 1 ? 1 : Math.max(0, easeBack(t)));
      graph.mergeNodeAttributes(id, a);
      if (t >= 1) {
        motions.delete(id);
        m.done?.();
      }
    }
    growing = growing.filter((g) => {
      if (now < g.start + g.dur) return true;
      if (!g.retract && graph!.hasEdge(g.key)) graph!.setEdgeAttribute(g.key, 'visibility', g.visible ? 'visible' : 'hidden');
      return false;
    });
    drawOverlay();
    if (motions.size || growing.length) animRaf = requestAnimationFrame(animate);
  }
  const play = () => {
    if (!animRaf) animRaf = requestAnimationFrame(animate);
  };

  function moveTo(id: string, x: number, y: number, dur = 500) {
    const a = graph!.getNodeAttributes(id) as { x: number; y: number };
    if (reducedMotion) graph!.mergeNodeAttributes(id, { x, y });
    else motions.set(id, { x0: a.x, y0: a.y, x1: x, y1: y, start: performance.now(), dur, ripple: false, size: motions.get(id)?.size });
    play();
  }

  function birth(e: Extract<Received['event'], { kind: 'store' }>) {
    const g = graph!;
    const near = e.edges.filter((x) => g.hasNode(x.to)).map((x) => g.getNodeAttributes(x.to) as { x: number; y: number });
    const cx = near.length ? near.reduce((a, p) => a + p.x, 0) / near.length + 4 : 0;
    const cy = near.length ? near.reduce((a, p) => a + p.y, 0) / near.length + 4 : 0;
    const enc = nodeAttributes(theme, e.phase, e.edges.length, false);
    const now = performance.now();
    bornAt.set(e.id, now);
    const px = unitsPerPx();
    const angle = Math.random() * Math.PI * 2;
    const from = reducedMotion ? { x: cx, y: cy } : { x: cx + 40 * px * Math.cos(angle), y: cy + 40 * px * Math.sin(angle) };
    g.addNode(e.id, { ...enc, x: from.x, y: from.y, index: -1, size: reducedMotion ? enc.size : 0 });
    e.edges.forEach((x, i) => {
      if (!g.hasNode(x.to)) return;
      // A memory can link to the same neighbor more than once (one per type).
      const key = `live-${e.seq}-${x.to}-${x.edgeType}`;
      if (g.hasEdge(key)) return;
      const visible = edgeTypes.has(x.edgeType);
      g.addEdgeWithKey(key, e.id, x.to, { ...edgeAttributes(theme, x.edgeType, x.weight), type: x.edgeType, visibility: reducedMotion && visible ? 'visible' : 'hidden' });
      if (!reducedMotion && visible) growing.push({ from: e.id, to: x.to, start: now + 180 + i * 70, dur: 380, key, visible });
    });
    if (reducedMotion) return;
    motions.set(e.id, { x0: from.x, y0: from.y, x1: cx, y1: cy, start: now, dur: 700, ripple: false, size: enc.size });
    // Nudge the closest memories outward, unless the layout is still moving them.
    if (now - lastTick > 500) {
      const R = 170 * px;
      const close: { id: string; x: number; y: number; d: number }[] = [];
      g.forEachNode((id, a) => {
        if (id === e.id || a.visibility !== 'visible' || motions.has(id)) return;
        const d = Math.hypot((a.x as number) - cx, (a.y as number) - cy);
        if (d < R && d > 0) close.push({ id, x: a.x as number, y: a.y as number, d });
      });
      close.sort((a, b) => a.d - b.d);
      for (const c of close.slice(0, 40)) {
        const push = (3 + 9 * (1 - c.d / R)) * px;
        const ux = (c.x - cx) / c.d, uy = (c.y - cy) / c.d;
        motions.set(c.id, { x0: c.x, y0: c.y, x1: c.x + ux * push, y1: c.y + uy * push, start: now + 60, dur: 900, ripple: true });
      }
    }
    play();
  }

  /** The reverse of birth: links pull back into the memory, it shrinks away,
   *  and nearby memories drift in slightly before settling. */
  function death(id: string) {
    const g = graph!;
    if (motions.get(id)?.shrink) return; // already leaving
    const a = g.getNodeAttributes(id) as { x: number; y: number; size: number; visibility: string };
    if (reducedMotion || a.visibility !== 'visible') {
      g.setNodeAttribute(id, 'visibility', 'hidden');
      return;
    }
    const now = performance.now();
    g.forEachEdge(id, (key, attr, src, dst) => {
      if (attr.visibility !== 'visible') return;
      g.setEdgeAttribute(key, 'visibility', 'hidden');
      growing.push({ from: id, to: src === id ? dst : src, start: now, dur: 320, key, visible: false, retract: true });
    });
    const size = a.size;
    motions.set(id, {
      x0: a.x, y0: a.y, x1: a.x, y1: a.y, start: now + 220, dur: 420, ripple: false, size, shrink: true,
      done: () => g.hasNode(id) && g.mergeNodeAttributes(id, { visibility: 'hidden', size }),
    });
    if (now - lastTick > 500) {
      const px = unitsPerPx();
      const R = 170 * px;
      const close: { id: string; x: number; y: number; d: number }[] = [];
      g.forEachNode((n, b) => {
        if (n === id || b.visibility !== 'visible' || motions.has(n)) return;
        const d = Math.hypot((b.x as number) - a.x, (b.y as number) - a.y);
        if (d < R && d > 0) close.push({ id: n, x: b.x as number, y: b.y as number, d });
      });
      close.sort((p, q) => p.d - q.d);
      for (const c of close.slice(0, 40)) {
        const pull = (2 + 6 * (1 - c.d / R)) * px;
        motions.set(c.id, { x0: c.x, y0: c.y, x1: c.x - ((c.x - a.x) / c.d) * pull, y1: c.y - ((c.y - a.y) / c.d) * pull, start: now + 300, dur: 900, ripple: true });
      }
    }
    play();
  }

  function applyToGraph(e: Received['event']) {
    if (!graph) return;
    if (e.kind === 'store' && !graph.hasNode(e.id)) {
      birth(e);
      summaries.delete(e.id);
    } else if (e.kind === 'link' && graph.hasNode(e.src) && graph.hasNode(e.dst)) {
      const key = `live-${e.seq}`;
      const visible = edgeTypes.has(e.edgeType);
      // A new memory's first links grow out of it; others just appear.
      const now = performance.now();
      const young = [e.src, e.dst].find((id) => now - (bornAt.get(id) ?? -Infinity) < 4000);
      const grow = visible && young !== undefined && !reducedMotion;
      if (!graph.hasEdge(key)) {
        graph.addEdgeWithKey(key, e.src, e.dst, { ...edgeAttributes(theme, e.edgeType, e.weight), type: e.edgeType, visibility: visible && !grow ? 'visible' : 'hidden' });
        if (grow) {
          growing.push({ from: young!, to: young === e.src ? e.dst : e.src, start: now + growing.filter((g) => g.from === young).length * 70, dur: 380, key, visible });
          play();
        }
      }
      for (const id of [e.src, e.dst]) {
        if ((graph.getNodeAttribute(id, 'index') as number) !== -1) continue;
        const near = graph.neighbors(id).map((n) => graph!.getNodeAttributes(n) as { x: number; y: number });
        if (near.length) {
          // Glide to the new neighbors' centroid instead of jumping there.
          graph.setNodeAttribute(id, 'size', nodeAttributes(theme, 1, graph.degree(id), false).size);
          moveTo(id, near.reduce((a, p) => a + p.x, 0) / near.length + 4, near.reduce((a, p) => a + p.y, 0) / near.length + 4);
        }
      }
    } else if (e.kind === 'phase_change' && graph.hasNode(e.id)) {
      const i = graph.getNodeAttribute(e.id, 'index') as number;
      const enc = nodeAttributes(theme, e.to, i >= 0 ? snapshot.degree[i] : graph.degree(e.id), i >= 0 && snapshot.permastore[i] === 1);
      graph.mergeNodeAttributes(e.id, { ...enc, phase: e.to, visibility: phases.has(e.to) ? enc.visibility : 'hidden' });
    } else if (e.kind === 'forget' && graph.hasNode(e.id)) {
      death(e.id);
    }
  }

  // ── Camera controls exposed to the page ───────────────────────────────
  export function zoomIn() {
    sigma?.getCamera().zoomIn({ duration: 200 });
  }
  export function zoomOut() {
    sigma?.getCamera().zoomOut({ duration: 200 });
  }
  export function fit() {
    if (focus) onfocuschange?.(false);
    focus = null;
    beforeFocus = null;
    sigma?.getCamera().reset({ duration: 300 });
  }
  // The camera before entering the local graph, restored on exit.
  let beforeFocus: { x: number; y: number; ratio: number; angle: number } | null = null;
  /** Leave the local graph and return to the view from before it. */
  export function exitFocus() {
    if (!focus) return;
    focus = null;
    const cam = sigma?.getCamera();
    if (beforeFocus) cam?.animate(beforeFocus, { duration: 300 });
    else cam?.reset({ duration: 300 });
    beforeFocus = null;
    onfocuschange?.(false);
  }
  /** Show only `id`'s 2-hop neighborhood (double-click, "Local graph"). */
  export function focusNeighborhood(id: string) {
    if (!graph?.hasNode(id)) return;
    const near = new Set([id]);
    for (const a of graph.neighbors(id)) {
      near.add(a);
      for (const b of graph.neighbors(a)) near.add(b);
    }
    if (!focus) beforeFocus = { ...sigma!.getCamera().getState() };
    focus = near;
    onfocuschange?.(true);
    const pts = [...near].map((n) => graph!.getNodeAttributes(n) as { x: number; y: number });
    const cx = pts.reduce((s, p) => s + p.x, 0) / pts.length, cy = pts.reduce((s, p) => s + p.y, 0) / pts.length;
    const c = sigma!.graphToViewport({ x: cx, y: cy });
    const cam = sigma!.viewportToFramedGraph(c);
    sigma!.getCamera().animate({ x: cam.x, y: cam.y, ratio: Math.max(0.15, Math.min(1, pts.length / 120)) }, { duration: 300 });
  }
  export function center(id: string) {
    if (!graph?.hasNode(id) || !sigma) return;
    const p = sigma.viewportToFramedGraph(sigma.graphToViewport(graph.getNodeAttributes(id) as { x: number; y: number }));
    sigma.getCamera().animate({ x: p.x, y: p.y, ratio: Math.min(sigma.getCamera().getState().ratio, 0.5) }, { duration: 300 });
  }
  export function getSigma() {
    return sigma;
  }
  export function getLight() {
    return light;
  }

  // ── Lifecycle ─────────────────────────────────────────────────────────
  onMount(() => {
    theme = readTheme();
    let disposed = false;
    loadPositions(scopeKey).then((cached) => {
      if (disposed) return;
      const built = build(snapshot, cached);
      graph = built.g;
      reencode();
      sigma = new Sigma(graph, container, {
        primitives: {
          nodes: {
            shapes: [sdfCircle()],
            layers: [
              layerBorder({
                borders: [
                  { size: 1, mode: 'pixels', color: { attribute: 'ring' } },
                  { size: 0, fill: true, color: { attribute: 'color' } },
                ],
              }),
            ],
          },
          edges: {
            variables: {
              dashColor: { type: 'color', default: '#0000' },
              dashSize: { type: 'number', default: 0 },
              gapSize: { type: 'number', default: 0 },
            },
            paths: [pathLine()],
            layers: [
              layerPlain({ color: { attribute: 'color' } }),
              layerDashed({ dashColor: { attribute: 'dashColor' }, dashSize: { attribute: 'dashSize' }, gapSize: { attribute: 'gapSize' } }),
            ],
          },
        },
        styles: {
          nodes: [
            {
              x: { attribute: 'x' },
              y: { attribute: 'y' },
              color: { attribute: 'color' },
              size: { attribute: 'size' },
              visibility: { attribute: 'visibility' },
              labelVisibility: 'hidden',
            },
          ],
          edges: [{ color: { attribute: 'color' }, size: { attribute: 'size' }, visibility: { attribute: 'visibility' } }],
        },
        settings: {
          renderLabels: false,
          renderEdgeLabels: false,
          enableNodeDrag: true,
          autoRescale: true,
          itemSizesReference: 'screen',
          zoomToSizeRatioFunction: (r: number) => Math.sqrt(r),
          minCameraRatio: 0.03,
          maxCameraRatio: 8,
          stagePadding: 40,
          doubleClickZoomingRatio: 1.6,
          hideEdgesOnMove: false,
        },
      } as never);

      sigma.on('afterRender', drawOverlay);
      // Continuous zoom: sigma steps a fixed ratio per wheel event and drops
      // events that come close together, which makes trackpad pinches lurch.
      // Scale by each event's own delta instead, anchored at the cursor.
      sigma.getMouseCaptor().on('wheel', (e) => {
        e.preventSigmaDefault();
        const ev = e.original as WheelEvent;
        ev.preventDefault();
        const dy = ev.deltaY * (ev.deltaMode === 1 ? 16 : ev.deltaMode === 2 ? 400 : 1);
        // Pinch arrives as ctrl+wheel with small deltas; mouse wheels send ~100 per notch.
        const factor = Math.exp(dy * (ev.ctrlKey ? 0.01 : 0.002));
        const cam = sigma!.getCamera();
        const ratio = cam.getBoundedRatio(cam.getState().ratio * factor);
        if (ratio !== cam.getState().ratio) cam.setState(sigma!.getViewportZoomedState({ x: e.x, y: e.y }, ratio));
      });
      try {
        light = new LightLayer(lightCanvas, () => ({
          m: affine((p) => sigma!.graphToViewport(p)),
          scale: 1 / Math.sqrt(sigma!.getCamera().getState().ratio),
        }));
        // Keep the light aligned while the camera moves mid-animation.
        sigma.on('afterRender', () => light && !light.animating && light.draw());
      } catch {
        light = null; // no WebGL2: the map still works, without light
      }
      unsubscribe = live.subscribe(onLive);
      sigma.on('enterNode', ({ node }) => {
        hovered = node;
        ensureSummaries([node]);
        clearTimeout(hoverTimer);
        // Tooltip after --dur-fast (120 ms).
        hoverTimer = setTimeout(() => {
          const p = sigma!.graphToViewport(graph!.getNodeAttributes(node) as { x: number; y: number });
          tooltip = { id: node, x: p.x, y: p.y, flip: p.x > container.clientWidth - 320 };
        }, 120);
        sigma!.refresh({ skipIndexation: true });
      });
      sigma.on('leaveNode', () => {
        hovered = null;
        tooltip = null;
        clearTimeout(hoverTimer);
        sigma!.refresh({ skipIndexation: true });
      });
      sigma.on('clickNode', ({ node }) => onselect(node));
      sigma.on('clickStage', () => onselect(null));
      sigma.on('doubleClickNode', ({ node, preventSigmaDefault }) => {
        preventSigmaDefault();
        focusNeighborhood(node);
      });

      // Physics lives in the worker; the main thread only copies positions.
      worker = new LayoutWorker();
      const apply = (x: Float32Array, y: Float32Array) => {
        lastTick = performance.now();
        graph!.updateEachNodeAttributes((_, a) => {
          const i = a.index as number;
          if (i < 0) return a; // added live; the worker doesn't simulate it
          a.x = x[i];
          a.y = y[i];
          return a;
        }, { attributes: ['x', 'y'] });
      };
      let first = true;
      let frozen = false;
      // Read before `known` is transferred (and detached) to the worker.
      const cachedAll = built.known.every((k) => k === 1);
      worker.onmessage = (e: MessageEvent<FromWorker>) => {
        apply(e.data.x, e.data.y);
        if (e.data.type === 'settled') {
          // Freeze the view's scale once the layout has settled, so dragging a
          // node outward no longer rescales the whole map.
          if (!frozen) {
            frozen = true;
            sigma!.setSetting('autoRescale', 'once');
          }
          savePositions(scopeKey, snapshot.ids, e.data.x, e.data.y);
          perf.__recalldSettled = true;
          onsettled?.();
        }
        if (first) {
          first = false;
          const cached = cachedAll;
          // Marks read by the perf harness (ui/perf); cheap and inert otherwise.
          performance.mark('recalld:map-ready');
          perf.__recalldCached = cached;
          onready?.({ cached, ms: performance.now() - started });
        }
      };
      sigma.on('nodeDragStart', ({ node }) => {
        const a = graph!.getNodeAttributes(node);
        worker!.postMessage({ type: 'drag', phase: 'start', index: a.index, x: a.x, y: a.y } satisfies ToWorker);
      });
      sigma.on('nodeDrag', ({ node }) => {
        const a = graph!.getNodeAttributes(node);
        worker!.postMessage({ type: 'drag', phase: 'move', index: a.index, x: a.x, y: a.y } satisfies ToWorker);
      });
      sigma.on('nodeDragEnd', ({ node }) => {
        const a = graph!.getNodeAttributes(node);
        worker!.postMessage({ type: 'drag', phase: 'end', index: a.index, x: a.x, y: a.y } satisfies ToWorker);
      });
      const src = snapshot.src.slice(), dst = snapshot.dst.slice(), weight = snapshot.weight.slice(), degree = snapshot.degree.slice();
      worker.postMessage(
        { type: 'init', n: snapshot.ids.length, src, dst, weight, degree, x: built.x, y: built.y, known: built.known, maxDegree: Math.max(1, ...degree) } satisfies ToWorker,
        [src.buffer, dst.buffer, weight.buffer, degree.buffer, built.x.buffer, built.y.buffer, built.known.buffer],
      );
    });
    return () => {
      disposed = true;
    };
  });

  let unsubscribe: (() => void) | null = null;
  onDestroy(() => {
    unsubscribe?.();
    light?.dispose();
    worker?.postMessage({ type: 'stop' } satisfies ToWorker);
    worker?.terminate();
    cancelAnimationFrame(animRaf);
    sigma?.kill();
  });

  const tipIndex = $derived(tooltip ? snapshot.index.get(tooltip.id) : undefined);
</script>

<div class="map" bind:this={container} role="application" aria-label="Memory map: {snapshot.ids.length} memories. Use the list below to reach them with the keyboard.">
  <canvas class="light" bind:this={lightCanvas} aria-hidden="true"></canvas>
  <canvas class="overlay" bind:this={overlay} aria-hidden="true"></canvas>
  {#if tooltip && tipIndex !== undefined}
    {@const i = tipIndex}
    {@const text = summaries.get(tooltip.id)}
    {@const r = retrievability(Math.max(0, Date.now() - snapshot.lastAccess[i]) / DAY_MS, snapshot.stability[i])}
    <div class="tooltip" role="tooltip" class:flip={tooltip.flip} style:left="{tooltip.x}px" style:top="{tooltip.y}px">
      <div class="t1">{text === null ? 'Summary dropped' : (text ?? '…')}</div>
      <div class="t2 mono">{phaseName(snapshot.phase[i])} · R {r.toFixed(2)} · S {duration(snapshot.stability[i])}</div>
      <div class="t2 mono">{snapshot.namespaces.find((n) => n.id === snapshot.ns[i])?.name ?? ''} · {snapshot.degree[i]} links · accessed {ago(snapshot.lastAccess[i])}</div>
    </div>
  {/if}
</div>
<ul class="visually-hidden" aria-label="Memories on the map">
  {#each visibleIds as id (id)}
    <li><button type="button" onclick={() => onselect(id)} onfocus={() => center(id)}>{summaries.get(id) ?? 'Summary dropped'}</button></li>
  {/each}
</ul>

<style>
  .map {
    position: absolute;
    inset: 0;
    background-color: var(--color-canvas);
    background-image: radial-gradient(var(--color-canvas-dot) var(--hairline), transparent var(--hairline));
    background-size: var(--space-6) var(--space-6);
  }
  .light {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    z-index: 1;
  }
  .overlay {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    z-index: 2;
  }
  .tooltip {
    position: absolute;
    z-index: 3;
    transform: translate(var(--space-4), calc(var(--space-4) * -1));
    max-width: calc(var(--panel-width) - var(--space-8));
    padding: calc(var(--space-3) + var(--hairline));
    background: var(--color-surface);
    border: var(--hairline) solid var(--color-border);
    border-radius: calc(var(--radius-md) + var(--hairline));
    box-shadow: var(--shadow-tooltip);
    pointer-events: none;
  }
  .tooltip.flip {
    transform: translate(calc(-100% - var(--space-4)), calc(var(--space-4) * -1));
  }
  .t1 {
    font: var(--weight-semibold) var(--text-control) var(--font-sans);
  }
  .t2 {
    font-size: var(--text-status);
    color: var(--color-text-muted);
  }
</style>

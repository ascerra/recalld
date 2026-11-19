// Turns one activity event into light (DESIGN §7). Pure, so its timing rules
// are unit-tested: pulses fire in PQSA order, ≈350 ms per hop scaled by edge
// length, brightness follows the spread amount and fades with each hop.
import type { UiEvent } from '$lib/api/types';
import type { Glow, Pulse } from './light';

export const HOP_MS = 350;
const BLOOM_ATTACK = 80;
const BLOOM_DECAY = 1200;
export const RESULT_RING_MS = 4000;
const SUPPRESS_MS = 600;

export interface Choreo {
  pulses: Pulse[];
  glows: Glow[];
  /** Result rings: id → expiry (ms, performance.now clock); `linked` gets the moss tint. */
  rings: { id: string; until: number; linked: boolean }[];
  /** Ids to label while lit (seeds first, then results). */
  labels: string[];
  suppressed: { id: string; until: number }[];
  /** Nodes to flash, with when the light reaches them (performance.now clock). */
  flashes: { id: string; at: number }[];
}

export interface Colors {
  agent: [number, number, number];
  lit: [number, number, number];
  moss: [number, number, number];
  cool: [number, number, number];
}

export function choreograph(
  event: UiEvent,
  pos: (id: string) => { x: number; y: number } | null,
  now: number,
  colors: Colors,
  opts: { speed?: number; reducedMotion?: boolean; scale?: number } = {},
): Choreo {
  const speed = opts.speed ?? 1;
  const out: Choreo = { pulses: [], glows: [], rings: [], labels: [], suppressed: [], flashes: [] };
  const glowAt = (id: string, start: number, intensity: number, color = colors.lit, kind: 0 | 1 | 2 = 0, radius = 22, decay = BLOOM_DECAY) => {
    const p = pos(id);
    if (p) out.glows.push({ x: p.x, y: p.y, start, attack: BLOOM_ATTACK * speed, decay: decay * speed, intensity: Math.min(1, intensity), radius, color, kind });
  };

  if (event.kind === 'recall') {
    // 1. Seed bloom: intensity = seed score.
    for (const s of event.seeds) {
      glowAt(s.id, now, 0.5 + 0.5 * s.score);
      out.flashes.push({ id: s.id, at: now });
    }
    // 2. Pulses along each traced edge, in firing order.
    let last = now;
    const steps = [...event.trace].sort((a, b) => a.order - b.order);
    for (const step of steps) {
      if (step.edgeType === 4) continue; // contradicts edges never carry light
      const a = pos(step.from), b = pos(step.to);
      if (!a || !b) continue;
      const len = Math.hypot(b.x - a.x, b.y - a.y) * (opts.scale ?? 1);
      const duration = HOP_MS * speed * Math.min(1.6, Math.max(0.6, len / 120));
      const start = now + step.order * HOP_MS * speed;
      const fade = Math.pow(0.85, step.order);
      if (!opts.reducedMotion) {
        out.pulses.push({ x0: a.x, y0: a.y, x1: b.x, y1: b.y, start, duration, width: 2.5 + 5 * step.amount, brightness: Math.min(1, (0.65 + step.amount) * fade), color: colors.agent });
      }
      // 3. Convergence: every arrival adds glow at the target (clamped in the shader).
      const arrive = start + (opts.reducedMotion ? 0 : duration);
      glowAt(step.to, arrive, 0.35 + 0.6 * step.amount * fade);
      out.flashes.push({ id: step.to, at: arrive });
      last = Math.max(last, start + duration);
    }
    // 4. Results keep a ring; linked ones get the moss tint.
    const seeds = new Set(event.seeds.map((s) => s.id));
    for (const id of event.returned) out.rings.push({ id, until: last + RESULT_RING_MS, linked: !seeds.has(id) });
    out.labels = [...event.seeds.map((s) => s.id), ...event.returned.filter((id) => !seeds.has(id))];
    // 5. Suppressed competitors cool for 600 ms, then recover.
    for (const id of event.suppressed.slice(0, 12)) {
      glowAt(id, last, 0.5, colors.cool, 2, 14, SUPPRESS_MS);
      out.suppressed.push({ id, until: last + SUPPRESS_MS * speed });
    }
  } else if (event.kind === 'store') {
    glowAt(event.id, now, 0.7, colors.lit, 0, 12, 300);
  } else if (event.kind === 'link') {
    const a = pos(event.src), b = pos(event.dst);
    if (a && b && !opts.reducedMotion)
      out.pulses.push({ x0: a.x, y0: a.y, x1: b.x, y1: b.y, start: now, duration: 300 * speed, width: 1.2, brightness: 0.5, color: colors.agent });
  } else if (event.kind === 'reinforce') {
    const gain = event.stabilityBefore && event.stabilityAfter ? Math.min(3, event.stabilityAfter / Math.max(0.1, event.stabilityBefore)) : 1.5;
    glowAt(event.id, now, 0.8);
    glowAt(event.id, now, 0.9, colors.agent, 1, 8 + 6 * gain, 900);
  }
  return out;
}

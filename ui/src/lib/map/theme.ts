// Map colors, read from the design tokens at runtime so tokens.css stays the
// only source of values (WebGL needs concrete colors).
export interface MapTheme {
  node: string;
  /** Per-namespace node tones; a namespace keeps its tone in every view. */
  tones: string[];
  nodeLit: string;
  edge: string;
  contradicts: string;
  permastore: string;
  text: string;
  accent: string;
  suppressed: string;
  halo: string;
  canvasDot: string;
  moss: string;
  agents: string[];
  fontSans: string;
}

const css = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

export function readTheme(): MapTheme {
  return {
    node: css('--color-node'),
    tones: [css('--color-node'), ...[2, 3, 4, 5, 6].map((i) => css(`--color-node-${i}`))],
    nodeLit: css('--color-node-lit'),
    edge: css('--color-edge'),
    contradicts: css('--color-contradicts'),
    permastore: css('--color-permastore'),
    text: css('--color-text'),
    accent: css('--color-accent'),
    suppressed: css('--color-suppressed'),
    halo: css('--color-label-halo'),
    canvasDot: css('--color-canvas-dot'),
    moss: css('--color-agent-2'),
    agents: [1, 2, 3, 4].map((i) => css(`--color-agent-${i}`)),
    fontSans: css('--font-sans'),
  };
}

/** CSS variable holding namespace `id`'s node tone (matches `MapTheme.tones`). */
export const toneVar = (id: number) => (id % 6 === 0 ? '--color-node' : `--color-node-${(id % 6) + 1}`);

/** Lighten (k > 0) or darken (k < 0) `#RRGGBB` by mixing toward white or black. */
export function shade(hex: string, k: number): string {
  const n = parseInt(hex.slice(1), 16);
  const t = k > 0 ? 255 : 0, a = Math.abs(k);
  const c = [n >> 16, (n >> 8) & 255, n & 255].map((v) => Math.round(v + (t - v) * a));
  return '#' + c.map((v) => v.toString(16).padStart(2, '0')).join('');
}

/** `#RRGGBB` + alpha → `rgba(r, g, b, a)`. */
export function alpha(hex: string, a: number): string {
  const h = hex.replace('#', '');
  const n = parseInt(h.length === 3 ? h.replace(/./g, (c) => c + c) : h, 16);
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${a})`;
}

export function rgb(hex: string): [number, number, number] {
  const h = hex.replace('#', '');
  const n = parseInt(h, 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

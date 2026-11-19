// Map colors, read from the design tokens at runtime so tokens.css stays the
// only source of values (WebGL needs concrete colors).
export interface MapTheme {
  node: string;
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

// The color palettes Honk offers. Each one has a light and a dark version, and the theme
// (System, Light, Dark) picks between them. Colors live in `src/app.css`, keyed by `id`.
// No imports, so tests/contrast.test.mjs can read this file directly.

export const PALETTES = [
  { id: "rhodonite", label: "Rhodonite" },
  { id: "catppuccin-mocha", label: "Catppuccin Mocha" },
  { id: "catppuccin-macchiato", label: "Catppuccin Macchiato" },
  { id: "catppuccin-frappe", label: "Catppuccin Frappé" },
] as const;

export type PaletteId = (typeof PALETTES)[number]["id"];

export const DEFAULT_PALETTE: PaletteId = "rhodonite";

export function isPaletteId(value: unknown): value is PaletteId {
  return PALETTES.some((palette) => palette.id === value);
}

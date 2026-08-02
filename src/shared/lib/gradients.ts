import type { StringKey } from "@/shared/i18n";

/**
 * Note colours.
 *
 * Only the identifier travels to SQLite (`notes.color`); the CSS lives in
 * `global.css` under `[data-note="..."]`, so that repainting the palette never
 * requires a migration. An identifier the UI no longer knows resolves to `null`,
 * which renders as a plain card rather than breaking the screen — that is what
 * makes removing a preset safe.
 *
 * A preset is only a pair of hues. How light, how saturated and what ink sits on
 * top are decided by the *theme*, not by the preset: a card that is a dark slab
 * on the white theme and a glowing block on the true-black one is the same bug
 * seen twice, and it cannot be fixed by choosing better fixed colours.
 */

export interface NoteGradient {
  readonly id: string;
  /** Dictionary key; the label itself lives in the locale files. */
  readonly labelKey: StringKey;
  /** Picker chip. Vivid on purpose: nothing has to stay legible on top of it. */
  readonly swatch: string;
}

export const NOTE_GRADIENTS: readonly NoteGradient[] = [
  {
    id: "sunset",
    labelKey: "color.sunset",
    swatch: "linear-gradient(135deg, oklch(72% 0.17 55) 0%, oklch(64% 0.19 8) 100%)",
  },
  {
    id: "ocean",
    labelKey: "color.ocean",
    swatch: "linear-gradient(135deg, oklch(70% 0.14 240) 0%, oklch(74% 0.13 195) 100%)",
  },
  {
    id: "forest",
    labelKey: "color.forest",
    swatch: "linear-gradient(135deg, oklch(72% 0.14 150) 0%, oklch(70% 0.12 185) 100%)",
  },
  {
    id: "lavender",
    labelKey: "color.lavender",
    swatch: "linear-gradient(135deg, oklch(70% 0.16 300) 0%, oklch(66% 0.16 268) 100%)",
  },
  {
    id: "rose",
    labelKey: "color.rose",
    swatch: "linear-gradient(135deg, oklch(71% 0.17 350) 0%, oklch(67% 0.16 318) 100%)",
  },
  {
    id: "amber",
    labelKey: "color.amber",
    swatch: "linear-gradient(135deg, oklch(80% 0.15 85) 0%, oklch(73% 0.16 52) 100%)",
  },
  {
    id: "mint",
    labelKey: "color.mint",
    swatch: "linear-gradient(135deg, oklch(78% 0.12 175) 0%, oklch(72% 0.12 212) 100%)",
  },
  {
    id: "graphite",
    labelKey: "color.graphite",
    swatch: "linear-gradient(135deg, oklch(62% 0.020 255) 0%, oklch(48% 0.016 255) 100%)",
  },
];

const BY_ID = new Map(NOTE_GRADIENTS.map((gradient) => [gradient.id, gradient]));

/** Resolves a stored identifier, tolerating values this build does not know. */
export function findGradient(id: string | null): NoteGradient | null {
  return id === null ? null : (BY_ID.get(id) ?? null);
}

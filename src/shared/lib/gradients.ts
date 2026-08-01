/**
 * Note colours.
 *
 * Only the identifier travels to SQLite (`notes.color`); the CSS lives here so
 * that repainting the palette never requires a migration. An identifier the UI
 * no longer knows resolves to `null`, which renders as a plain card rather than
 * breaking the screen — that is what makes removing a preset safe.
 */

export interface NoteGradient {
  readonly id: string;
  readonly label: string;
  /** Card background. Muted on purpose: near-white body text sits on top. */
  readonly surface: string;
  /** Vivid version for the picker chip, where nothing has to stay legible. */
  readonly swatch: string;
  /** Hairline that keeps a coloured card from bleeding into the background. */
  readonly border: string;
}

export const NOTE_GRADIENTS: readonly NoteGradient[] = [
  {
    id: "sunset",
    label: "Закат",
    surface: "linear-gradient(135deg, oklch(36% 0.082 45) 0%, oklch(31% 0.074 12) 100%)",
    swatch: "linear-gradient(135deg, oklch(72% 0.17 55) 0%, oklch(64% 0.19 8) 100%)",
    border: "oklch(46% 0.075 30)",
  },
  {
    id: "ocean",
    label: "Океан",
    surface: "linear-gradient(135deg, oklch(35% 0.070 235) 0%, oklch(31% 0.068 195) 100%)",
    swatch: "linear-gradient(135deg, oklch(70% 0.14 240) 0%, oklch(74% 0.13 195) 100%)",
    border: "oklch(45% 0.068 220)",
  },
  {
    id: "forest",
    label: "Лес",
    surface: "linear-gradient(135deg, oklch(35% 0.066 155) 0%, oklch(31% 0.060 185) 100%)",
    swatch: "linear-gradient(135deg, oklch(72% 0.14 150) 0%, oklch(70% 0.12 185) 100%)",
    border: "oklch(45% 0.062 165)",
  },
  {
    id: "lavender",
    label: "Лаванда",
    surface: "linear-gradient(135deg, oklch(36% 0.078 300) 0%, oklch(32% 0.072 265) 100%)",
    swatch: "linear-gradient(135deg, oklch(70% 0.16 300) 0%, oklch(66% 0.16 268) 100%)",
    border: "oklch(46% 0.072 285)",
  },
  {
    id: "rose",
    label: "Роза",
    surface: "linear-gradient(135deg, oklch(36% 0.080 350) 0%, oklch(32% 0.072 320) 100%)",
    swatch: "linear-gradient(135deg, oklch(71% 0.17 350) 0%, oklch(67% 0.16 318) 100%)",
    border: "oklch(46% 0.074 338)",
  },
  {
    id: "amber",
    label: "Янтарь",
    surface: "linear-gradient(135deg, oklch(37% 0.072 80) 0%, oklch(33% 0.070 50) 100%)",
    swatch: "linear-gradient(135deg, oklch(80% 0.15 85) 0%, oklch(73% 0.16 52) 100%)",
    border: "oklch(47% 0.070 68)",
  },
  {
    id: "mint",
    label: "Мята",
    surface: "linear-gradient(135deg, oklch(36% 0.058 175) 0%, oklch(32% 0.062 210) 100%)",
    swatch: "linear-gradient(135deg, oklch(78% 0.12 175) 0%, oklch(72% 0.12 212) 100%)",
    border: "oklch(46% 0.058 192)",
  },
  {
    id: "graphite",
    label: "Графит",
    surface: "linear-gradient(135deg, oklch(32% 0.014 255) 0%, oklch(27% 0.012 255) 100%)",
    swatch: "linear-gradient(135deg, oklch(62% 0.020 255) 0%, oklch(48% 0.016 255) 100%)",
    border: "oklch(42% 0.014 255)",
  },
];

const BY_ID = new Map(NOTE_GRADIENTS.map((gradient) => [gradient.id, gradient]));

/** Resolves a stored identifier, tolerating values this build does not know. */
export function findGradient(id: string | null): NoteGradient | null {
  return id === null ? null : (BY_ID.get(id) ?? null);
}

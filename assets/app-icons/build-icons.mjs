// Генерирует стилевые варианты иконки приложения (SVG + PNG превью).
//
//   node assets/app-icons/build-icons.mjs
//
// Геометрия знака взята один в один из исходника xima.keeps-*.svg (Figma):
// меняется только оформление — подложка, рамка, заливка знака.
// SVG — источник правды; PNG рендерятся из него headless-Chrome и нужны
// только для превью, иконки платформ собираются отдельно (`tauri icon`).

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const svgDir = join(here, "svg");
const pngDir = join(here, "png");

const CANVAS = 1096;
const PREVIEW = 512;

const BODY_PATH =
  "M364.82 386.948C328.848 396.587 307.501 433.561 317.14 469.532L386.948 730.061C396.586 766.032 433.561 787.379 469.532 777.741L621.168 737.11L592.263 629.235C582.624 593.264 603.971 556.289 639.943 546.651L748.835 517.473L707.932 364.82C698.294 328.849 661.319 307.501 625.348 317.14L364.82 386.948Z";
const FLAP_PATH =
  "M635.75 616.49C632.588 604.686 639.592 592.554 651.395 589.392L760.288 560.214C766.072 581.802 763.042 604.806 751.868 624.173L716.853 684.821C705.667 704.181 687.261 718.307 665.673 724.092L664.655 724.365L635.75 616.49Z";

/**
 * Оформление одного варианта.
 * @typedef {object} Style
 * @property {string} base          сплошная заливка подложки
 * @property {[string,string]} glow радиальный блик поверх подложки (от/до)
 * @property {[string,string]} border градиент внутренней рамки
 * @property {string} mark          заливка тела знака
 * @property {string} flap          заливка отогнутого уголка
 * @property {string} shadowRGB     цвет тени знака как "r g b" в долях 0..1
 * @property {string} [defs]        дополнительные определения (градиенты знака)
 */

function render(id, style) {
  const s = { glowOpacity: 1, ...style };
  return `<svg width="${CANVAS}" height="${CANVAS}" viewBox="0 0 ${CANVAS} ${CANVAS}" fill="none" xmlns="http://www.w3.org/2000/svg">
<g clip-path="url(#clip_${id})">
<rect width="${CANVAS}" height="${CANVAS}" fill="${s.base}"/>
<rect width="${CANVAS}" height="${CANVAS}" fill="url(#glow_${id})"/>
<rect x="14.9844" y="14.9844" width="1066.03" height="1066.03" stroke="url(#border_${id})" stroke-width="29.9688"/>
<g filter="url(#shadow_${id})">
<path d="${BODY_PATH}" fill="${s.mark}"${s.markStroke ?? ""}/>
<g filter="url(#inner_${id})">
<path d="${FLAP_PATH}" fill="${s.flap}"${s.markStroke ?? ""}/>
</g>
</g>
</g>
<defs>
${s.defs ?? ""}<filter id="shadow_${id}" x="235.622" y="246.325" width="602.459" height="619.354" filterUnits="userSpaceOnUse" color-interpolation-filters="sRGB">
<feFlood flood-opacity="0" result="BackgroundImageFix"/>
<feColorMatrix in="SourceAlpha" type="matrix" values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 127 0" result="hardAlpha"/>
<feOffset dx="-2.14062" dy="8.5625"/>
<feGaussianBlur stdDeviation="38.5312"/>
<feComposite in2="hardAlpha" operator="out"/>
<feColorMatrix type="matrix" values="0 0 0 0 ${s.shadowRGB.split(" ").join(" 0 0 0 0 ")} 0 0 0 ${s.shadowOpacity ?? 0.25} 0"/>
<feBlend mode="normal" in2="BackgroundImageFix" result="dropShadow"/>
<feBlend mode="normal" in="SourceGraphic" in2="dropShadow" result="shape"/>
</filter>
<filter id="inner_${id}" x="634.991" y="560.214" width="134.59" height="172.713" filterUnits="userSpaceOnUse" color-interpolation-filters="sRGB">
<feFlood flood-opacity="0" result="BackgroundImageFix"/>
<feBlend mode="normal" in="SourceGraphic" in2="BackgroundImageFix" result="shape"/>
<feColorMatrix in="SourceAlpha" type="matrix" values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 127 0" result="hardAlpha"/>
<feOffset dx="6.42188" dy="8.5625"/>
<feGaussianBlur stdDeviation="4.28125"/>
<feComposite in2="hardAlpha" operator="arithmetic" k2="-1" k3="1"/>
<feColorMatrix type="matrix" values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 ${s.innerOpacity ?? 0.25} 0"/>
<feBlend mode="normal" in2="shape" result="innerShadow"/>
</filter>
<radialGradient id="glow_${id}" cx="0" cy="0" r="1" gradientUnits="userSpaceOnUse" gradientTransform="translate(775.977 792.031) rotate(126.87) scale(379.961)">
<stop stop-color="${s.glow[0]}"/>
<stop offset="1" stop-color="${s.glow[1]}"/>
</radialGradient>
<linearGradient id="border_${id}" x1="1048.91" y1="52.4453" x2="84.5546" y2="1037.13" gradientUnits="userSpaceOnUse">
<stop stop-color="${s.border[0]}"/>
<stop offset="1" stop-color="${s.border[1]}"/>
</linearGradient>
<clipPath id="clip_${id}">
<rect width="${CANVAS}" height="${CANVAS}" fill="white"/>
</clipPath>
</defs>
</svg>
`;
}

const variants = [
  {
    id: "ink",
    name: "Ink",
    description: "Исходный бренд: графитовый знак на светлой подложке.",
    accent: "#222020",
    style: {
      base: "#222020",
      glow: ["#E7E9F1", "#FFFFFF"],
      border: ["#F3F3F3", "#D9D9D9"],
      mark: "#222020",
      flap: "#222020",
      shadowRGB: "0.0294 0.0294 0.0294",
    },
  },
  {
    id: "amber",
    name: "Amber",
    description: "Тёплая бумага и янтарный знак — «стикер на столе».",
    accent: "#E8862B",
    style: {
      base: "#FFF6E8",
      glow: ["#FFE6BE", "#FFFBF3"],
      border: ["#FFD9A1", "#E8B673"],
      mark: "url(#markAmber)",
      flap: "#C96F1E",
      shadowRGB: "0.36 0.19 0.03",
      shadowOpacity: 0.3,
      defs: `<linearGradient id="markAmber" x1="317" y1="317" x2="749" y2="778" gradientUnits="userSpaceOnUse">
<stop stop-color="#F9A94B"/>
<stop offset="1" stop-color="#D9701A"/>
</linearGradient>
`,
    },
  },
  {
    id: "midnight",
    name: "Midnight",
    description: "Тёмная подложка и мятный знак — под тёмную тему системы.",
    accent: "#4BE3B0",
    style: {
      base: "#0F141A",
      glow: ["#1E2A33", "#0C1116"],
      border: ["#2C3B46", "#141B21"],
      mark: "url(#markMidnight)",
      flap: "#2AA97F",
      shadowRGB: "0.29 0.89 0.69",
      shadowOpacity: 0.45,
      innerOpacity: 0.35,
      defs: `<linearGradient id="markMidnight" x1="317" y1="317" x2="749" y2="778" gradientUnits="userSpaceOnUse">
<stop stop-color="#6BF3C4"/>
<stop offset="1" stop-color="#2FB98C"/>
</linearGradient>
`,
    },
  },
  {
    id: "paper",
    name: "Paper",
    description: "Контурный знак на кремовом фоне — самый лёгкий вариант.",
    accent: "#8A6A4B",
    style: {
      base: "#F7F1E6",
      glow: ["#EFE5D2", "#FBF7F0"],
      border: ["#E6D9C2", "#CBB899"],
      mark: "none",
      flap: "none",
      markStroke: ' stroke="#8A6A4B" stroke-width="26" stroke-linejoin="round"',
      shadowRGB: "0.54 0.42 0.29",
      shadowOpacity: 0.18,
      innerOpacity: 0,
    },
  },
  {
    id: "neon",
    name: "Neon",
    description: "Индиго-фиолетовый градиент и светлый знак с неоновой тенью.",
    accent: "#7C5CFF",
    style: {
      base: "#2A1B63",
      glow: ["#7B3FD8", "#2A1B63"],
      border: ["#B071FF", "#4B2AA8"],
      mark: "url(#markNeon)",
      flap: "#D9C7FF",
      shadowRGB: "1 0.48 0.9",
      shadowOpacity: 0.55,
      defs: `<linearGradient id="markNeon" x1="317" y1="317" x2="749" y2="778" gradientUnits="userSpaceOnUse">
<stop stop-color="#FFFFFF"/>
<stop offset="1" stop-color="#E9DDFF"/>
</linearGradient>
`,
    },
  },
];

mkdirSync(svgDir, { recursive: true });
mkdirSync(pngDir, { recursive: true });

for (const variant of variants) {
  writeFileSync(join(svgDir, `${variant.id}.svg`), render(variant.id, variant.style));
}

writeFileSync(
  join(here, "variants.json"),
  `${JSON.stringify(
    variants.map(({ id, name, description, accent }) => ({
      id,
      name,
      description,
      accent,
      svg: `svg/${id}.svg`,
      preview: `png/${id}.png`,
    })),
    null,
    2,
  )}\n`,
);

const chrome = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
if (!existsSync(chrome)) {
  console.log(`SVG собраны (${variants.length}). Chrome не найден — PNG-превью пропущены.`);
  process.exit(0);
}

// Chrome рендерит SVG в натуральном размере, а вьюпорт headless-окна ниже
// запрошенного (низ снимка остаётся пустым). Поэтому иконку центрируем в
// заведомо более высоком окне и вырезаем квадрат по центру через sips.
const WINDOW_HEIGHT = PREVIEW + 200;

for (const variant of variants) {
  const wrapper = join(pngDir, `.${variant.id}.html`);
  const png = join(pngDir, `${variant.id}.png`);
  writeFileSync(
    wrapper,
    `<!doctype html><meta charset="utf-8"><style>html,body{margin:0}img{position:absolute;left:0;top:${(WINDOW_HEIGHT - PREVIEW) / 2}px;width:${PREVIEW}px;height:${PREVIEW}px}</style><img src="../svg/${variant.id}.svg">`,
  );
  execFileSync(
    chrome,
    [
      "--headless=new",
      "--disable-gpu",
      "--hide-scrollbars",
      "--force-device-scale-factor=1",
      `--window-size=${PREVIEW},${WINDOW_HEIGHT}`,
      `--screenshot=${png}`,
      `file://${wrapper}`,
    ],
    { stdio: "ignore" },
  );
  execFileSync("sips", ["-c", `${PREVIEW}`, `${PREVIEW}`, png, "--out", png], {
    stdio: "ignore",
  });
  rmSync(wrapper);
}

console.log(`Готово: ${variants.length} варианта(ов), SVG + PNG-превью.`);

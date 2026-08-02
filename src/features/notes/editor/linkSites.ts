/**
 * Which icon a link wears.
 *
 * The mapping is a table of hosts rather than a favicon lookup: the app keeps
 * everything on the device, and asking a favicon service for an icon would send
 * every domain in your notes to a third party and still show nothing offline.
 * The cost is that only known services are recognised — everything else gets the
 * generic globe, which is the honest answer for a host we have no artwork for.
 *
 * The icons themselves live in `styles/linkIcons.css`, keyed by the names below.
 */

/** Suffix match, so `www.` and regional or app subdomains resolve too. */
const HOSTS: readonly (readonly [string, readonly string[]])[] = [
  ["telegram", ["t.me", "telegram.me", "telegram.org", "telegram.dog"]],
  ["whatsapp", ["whatsapp.com", "wa.me", "chat.whatsapp.com"]],
  ["instagram", ["instagram.com", "instagr.am"]],
  ["youtube", ["youtube.com", "youtu.be", "music.youtube.com"]],
  ["tiktok", ["tiktok.com", "vt.tiktok.com"]],
  ["vk", ["vk.com", "vk.ru", "vkvideo.ru"]],
  ["github", ["github.com", "gist.github.com"]],
  ["x", ["x.com", "twitter.com", "t.co"]],
  ["googlemaps", ["maps.google.com", "goo.gl/maps", "maps.app.goo.gl"]],
  ["spotify", ["spotify.com", "open.spotify.com"]],
  ["figma", ["figma.com"]],
  ["notion", ["notion.so", "notion.site"]],
  ["wikipedia", ["wikipedia.org", "wikimedia.org"]],
  // 2GIS and Yandex Maps have no mark in the icon set, so the whole category
  // shares one pin: a wrong brand logo would read worse than a plain category.
  ["map", ["2gis.ru", "2gis.com", "2gis.kz", "go.2gis.com", "yandex.ru/maps", "maps.yandex.ru"]],
];

/** The globe. Also what an unparseable href gets. */
export const FALLBACK_SITE = "link";

function hostMatches(host: string, pattern: string): boolean {
  const domain = pattern.split("/")[0] ?? pattern;
  return host === domain || host.endsWith(`.${domain}`);
}

/**
 * Names the icon for an href, or `null` when the mark carries no usable link —
 * an empty `href` must not paint a globe onto ordinary text.
 */
export function siteOf(href: unknown): string | null {
  if (typeof href !== "string" || href.trim() === "") {
    return null;
  }

  let url: URL;
  try {
    url = new URL(href, "https://example.invalid");
  } catch {
    return FALLBACK_SITE;
  }

  if (url.protocol === "mailto:") {
    return "mail";
  }
  if (url.protocol === "tel:") {
    return "phone";
  }

  const host = url.hostname.toLowerCase().replace(/^www\./, "");
  const path = `${host}${url.pathname}`;

  for (const [site, patterns] of HOSTS) {
    for (const pattern of patterns) {
      // A pattern with a path (`yandex.ru/maps`) must match the path too, so a
      // plain search link does not turn into a map pin.
      const matched = pattern.includes("/")
        ? path.startsWith(pattern)
        : hostMatches(host, pattern);
      if (matched) {
        return site;
      }
    }
  }

  return FALLBACK_SITE;
}

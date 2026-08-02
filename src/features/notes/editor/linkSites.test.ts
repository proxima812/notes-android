import { describe, expect, it } from "vitest";

import { FALLBACK_SITE, siteOf } from "./linkSites";

describe("siteOf", () => {
  it("recognises a service by host, including subdomains and www", () => {
    expect(siteOf("https://t.me/durov")).toBe("telegram");
    expect(siteOf("https://www.instagram.com/p/abc/")).toBe("instagram");
    expect(siteOf("https://music.youtube.com/watch?v=x")).toBe("youtube");
  });

  it("does not match a host that merely ends with the same letters", () => {
    // `nott.me` is not Telegram, and `notgithub.com` is not GitHub.
    expect(siteOf("https://nott.me/x")).toBe(FALLBACK_SITE);
    expect(siteOf("https://notgithub.com")).toBe(FALLBACK_SITE);
  });

  it("keeps a path-qualified pattern from swallowing the whole domain", () => {
    expect(siteOf("https://yandex.ru/maps/213/moscow/")).toBe("map");
    expect(siteOf("https://yandex.ru/search/?text=x")).toBe(FALLBACK_SITE);
  });

  it("maps 2GIS to the shared map pin", () => {
    expect(siteOf("https://2gis.ru/moscow/firm/70000001006559194")).toBe("map");
  });

  it("reads the scheme for mail and phone links", () => {
    expect(siteOf("mailto:someone@example.com")).toBe("mail");
    expect(siteOf("tel:+79991234567")).toBe("phone");
  });

  it("has nothing to draw without an href", () => {
    expect(siteOf("")).toBeNull();
    expect(siteOf(undefined)).toBeNull();
    expect(siteOf(null)).toBeNull();
  });
});

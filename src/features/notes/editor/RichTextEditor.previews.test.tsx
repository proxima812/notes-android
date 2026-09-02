import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/features/links/api", () => ({ fetchLinkPreview: vi.fn() }));

import { fetchLinkPreview as real } from "@/features/links/api";
import { RichTextEditor } from "./RichTextEditor";
import { forgetKnownPreviews } from "./linkPreviewStore";
import { I18nProvider } from "@/shared/i18n";

/** A document with one link whose text is still the raw address. */
function documentWith(href: string, text: string): string {
  return JSON.stringify({
    type: "doc",
    content: [
      {
        type: "paragraph",
        content: [{ type: "text", marks: [{ type: "link", attrs: { href } }], text }],
      },
    ],
  });
}

function open(json: string): void {
  render(
    <I18nProvider>
      <RichTextEditor initialJson={json} initialText="" onChange={() => {}} />
    </I18nProvider>,
  );
}

const fetchLinkPreview = vi.mocked(real);

/** The icon is a decoration inside the anchor, not an attribute on it. */
function iconStyleOf(): string {
  return screen.getByRole("link").querySelector("[data-link-icon]")?.getAttribute("style") ?? "";
}

function siteOf(): string | null {
  return (
    screen.getByRole("link").querySelector("[data-link-site]")?.getAttribute("data-link-site") ??
    null
  );
}

const ICON = "data:image/png;base64,AAAA";

describe("link previews in the editor", () => {
  beforeEach(() => {
    forgetKnownPreviews();
    fetchLinkPreview.mockReset();
  });

  it("replaces a pasted address with the title of the page", async () => {
    fetchLinkPreview.mockResolvedValue({
      url: "https://workos.com/",
      title: "WorkOS — Your app, Enterprise Ready.",
      icon: ICON,
      ok: true,
    });

    open(documentWith("https://workos.com/", "https://workos.com/"));

    await waitFor(() => {
      expect(screen.getByRole("link").textContent).toBe(
        "WorkOS — Your app, Enterprise Ready.",
      );
    });
    // The address itself is untouched — only what is shown for it changed.
    expect(screen.getByRole("link").getAttribute("href")).toBe("https://workos.com/");
  });

  it("draws the icon the site actually has", async () => {
    fetchLinkPreview.mockResolvedValue({
      url: "https://workos.com/",
      title: null,
      icon: ICON,
      ok: true,
    });

    open(documentWith("https://workos.com/", "https://workos.com/"));

    await waitFor(() => {
      expect(iconStyleOf()).toContain(ICON);
    });
  });

  /// Somebody's own words are theirs. A title must never overwrite them.
  it("leaves a link that already has a name of its own", async () => {
    fetchLinkPreview.mockResolvedValue({
      url: "https://workos.com/",
      title: "WorkOS — Your app, Enterprise Ready.",
      icon: ICON,
      ok: true,
    });

    open(documentWith("https://workos.com/", "почитать про SSO"));

    await waitFor(() => {
      expect(iconStyleOf()).toContain(ICON);
    });
    expect(screen.getByRole("link").textContent).toBe("почитать про SSO");
  });

  it("keeps the hand-drawn mark when the site gives nothing back", async () => {
    fetchLinkPreview.mockResolvedValue({
      url: "https://t.me/durov",
      title: null,
      icon: null,
      ok: false,
    });

    open(documentWith("https://t.me/durov", "https://t.me/durov"));

    await waitFor(() => {
      expect(fetchLinkPreview).toHaveBeenCalled();
    });
    expect(siteOf()).toBe("telegram");
  });

  /// The bug this shape exists to prevent: the link is drawn first and the icon
  /// arrives afterwards. Nothing about the document changes when it does, so an
  /// icon rendered by the link mark would never appear — only a decoration is
  /// rebuilt on a state change that leaves the document alone.
  it("shows an icon that arrives after the link is already on screen", async () => {
    const waiting: (() => void)[] = [];
    fetchLinkPreview.mockImplementation(
      async () =>
        new Promise((resolve) => {
          waiting.push(() => {
            resolve({ url: "https://workos.com/", title: null, icon: ICON, ok: true });
          });
        }),
    );

    open(documentWith("https://workos.com/", "уже на экране"));

    // Drawn, and wearing the fallback mark, before anything has been answered.
    await waitFor(() => {
      expect(siteOf()).toBe("link");
    });
    expect(iconStyleOf()).toBe("");

    // Every ask is answered, so nothing is left holding a slot in the queue.
    for (const answer of waiting) {
      answer();
    }

    await waitFor(() => {
      expect(iconStyleOf()).toContain(ICON);
    });
  });

  // A different address from every other test here on purpose: this one counts
  // the asks, and a store shared between tests would let one test's answer
  // spare another test the question.
  it("asks about one address once, however many links point at it", async () => {
    fetchLinkPreview.mockResolvedValue({
      url: "https://asked-once.example/",
      title: "Asked once",
      icon: null,
      ok: true,
    });

    open(documentWith("https://asked-once.example/", "https://asked-once.example/"));

    await waitFor(() => {
      expect(screen.getByRole("link").textContent).toBe("Asked once");
    });
    expect(fetchLinkPreview).toHaveBeenCalledTimes(1);
  });
});

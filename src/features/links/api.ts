import { z } from "zod";

import { callCommand, emptyResponseSchema } from "@/shared/api/command";

export const linkPreviewSchema = z.object({
  url: z.string(),
  title: z.string().nullable(),
  /** A `data:` URL, so the icon needs no second request and works offline. */
  icon: z.string().nullable(),
  /** False when the last attempt did not reach the site. */
  ok: z.boolean(),
});

export type LinkPreview = z.infer<typeof linkPreviewSchema>;

/**
 * What an address is called and what its icon looks like.
 *
 * `null` for an address the core does not fetch — a `mailto:`, a phone number,
 * or text that is not a URL.
 */
export async function fetchLinkPreview(url: string): Promise<LinkPreview | null> {
  return callCommand("links_preview", linkPreviewSchema.nullable(), { url });
}

/** Empties the cache of pages the app has read. */
export async function forgetLinkPreviews(): Promise<null> {
  return callCommand("links_forget_all", emptyResponseSchema);
}

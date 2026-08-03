import { z } from "zod";

import { callCommand } from "@/shared/api/command";

export const appIconSchema = z.object({
  id: z.string().min(1),
  label: z.string().min(1),
  /** Six-digit hex, for the swatch behind the name. */
  accent: z.string().regex(/^#[0-9a-fA-F]{6}$/),
});

export const appIconCatalogSchema = z.object({
  selectedId: z.string().min(1),
  items: z.array(appIconSchema).min(1),
});

export type AppIcon = z.infer<typeof appIconSchema>;
export type AppIconCatalog = z.infer<typeof appIconCatalogSchema>;

export async function listAppIcons(): Promise<AppIconCatalog> {
  return callCommand("app_icons_list", appIconCatalogSchema);
}

/**
 * Switches the launcher icon.
 *
 * Android has no icon to set — it has components to enable, one per icon — so
 * the answer is the catalogue as it stands afterwards rather than a promise
 * that the change took.
 */
export async function selectAppIcon(id: string): Promise<AppIconCatalog> {
  return callCommand("app_icons_select", appIconCatalogSchema, { id });
}

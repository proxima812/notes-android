import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check } from "lucide-react";

import { describeError } from "@/shared/api/errors";
import { useT } from "@/shared/i18n";

import { listAppIcons, selectAppIcon } from "../api";

/**
 * The icon the app wears on the home screen.
 *
 * The list comes from the core rather than from here: every icon has to exist
 * as a component declared in the Android manifest, so a variant the build does
 * not ship could not be chosen even if this offered it.
 */
export function AppIconPicker(): React.JSX.Element | null {
  const t = useT();
  const client = useQueryClient();

  const catalog = useQuery({ queryKey: ["app-icons"], queryFn: listAppIcons });
  const choose = useMutation({
    mutationFn: selectAppIcon,
    onSuccess: (next) => {
      client.setQueryData(["app-icons"], next);
    },
  });

  if (catalog.data === undefined) {
    // Off-device the command is refused outright; a settings screen missing one
    // section reads better than one showing an error nobody can act on.
    return null;
  }

  return (
    <section className="flex flex-col gap-3">
      <h2 className="text-content-muted text-sm font-medium">{t("appIcon.title")}</h2>

      <div role="radiogroup" aria-label={t("appIcon.title")} className="grid grid-cols-2 gap-3">
        {catalog.data.items.map((icon) => {
          const selected = icon.id === catalog.data.selectedId;
          return (
            <button
              key={icon.id}
              type="button"
              role="radio"
              aria-checked={selected}
              disabled={choose.isPending}
              onClick={() => {
                choose.mutate(icon.id);
              }}
              className={`flex min-h-14 items-center gap-3 rounded-2xl border px-3 text-left disabled:opacity-40 ${
                selected ? "border-accent" : "border-border-subtle"
              }`}
            >
              {/* The artwork itself, not a swatch standing in for it: the
                  whole question the screen answers is what the icon looks
                  like. The accent sits behind it so the tile has its colour
                  before the image has decoded. */}
              <img
                src={`/app-icons/${icon.id}.png`}
                alt=""
                width={32}
                height={32}
                loading="lazy"
                className="border-border-subtle size-8 shrink-0 rounded-xl border object-cover"
                style={{ backgroundColor: icon.accent }}
              />
              <span className="text-content flex-1 text-sm font-medium">{icon.label}</span>
              {selected && <Check className="text-accent size-4 shrink-0" />}
            </button>
          );
        })}
      </div>

      {choose.error !== null && (
        <p role="alert" className="text-danger text-sm">
          {describeError(choose.error, t)}
        </p>
      )}

      <p className="text-content-muted/70 text-xs">{t("appIcon.hint")}</p>
    </section>
  );
}

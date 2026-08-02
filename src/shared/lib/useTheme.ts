import { useState } from "react";

import { applyTheme, loadTheme, saveTheme, type AppThemeId } from "./theme";

/**
 * The current theme and a setter that also paints and persists it.
 *
 * State is seeded from storage on mount rather than lifted into a context: the
 * screens that offer a picker are never mounted at the same time, and the paint
 * itself happens through a `<html>` attribute, so nothing else has to re-render
 * when the theme changes.
 */
export function useTheme(): readonly [AppThemeId, (id: AppThemeId) => void] {
  const [theme, setTheme] = useState<AppThemeId>(() => loadTheme());

  const choose = (id: AppThemeId): void => {
    setTheme(id);
    applyTheme(id);
    saveTheme(id);
  };

  return [theme, choose];
}

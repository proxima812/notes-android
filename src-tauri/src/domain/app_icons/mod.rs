//! The icons the app can wear on the home screen.
//!
//! The catalogue is compiled in rather than read from disk: every icon has to
//! exist as an Android component declared in a manifest, so a variant the build
//! does not ship could never be selected anyway. `assets/app-icons/variants.json`
//! is where the artwork and these entries are kept in step.

use crate::error::{AppError, AppResult, ValidationError};

pub const SELECTED_ICON_SETTING_KEY: &str = "appearance.app_icon";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppIcon {
    pub id: &'static str,
    /// Name of the `activity-alias`, which is what Android switches on and off.
    pub alias: &'static str,
    pub label: &'static str,
    /// Colour to show behind the name in settings, from the artwork.
    pub accent: &'static str,
}

/// The one enabled in the manifest, so a fresh install already has an icon.
///
/// The original mark, which is what the app wore before there was a choice —
/// picking anything else has to be something the user did on purpose.
pub const DEFAULT_ICON_ID: &str = "original";

pub const APP_ICONS: &[AppIcon] = &[
    AppIcon {
        id: "original",
        alias: "Original",
        label: "xima.keeps",
        accent: "#0B0B0D",
    },
    AppIcon {
        id: "ink",
        alias: "Ink",
        label: "Ink",
        accent: "#222020",
    },
    AppIcon {
        id: "amber",
        alias: "Amber",
        label: "Amber",
        accent: "#E8862B",
    },
    AppIcon {
        id: "midnight",
        alias: "Midnight",
        label: "Midnight",
        accent: "#4BE3B0",
    },
    AppIcon {
        id: "paper",
        alias: "Paper",
        label: "Paper",
        accent: "#8A6A4B",
    },
    AppIcon {
        id: "neon",
        alias: "Neon",
        label: "Neon",
        accent: "#7C5CFF",
    },
];

#[must_use]
pub const fn app_icons() -> &'static [AppIcon] {
    APP_ICONS
}

/// Every alias the build ships, for switching the others off.
#[must_use]
pub fn aliases() -> Vec<String> {
    APP_ICONS.iter().map(|icon| icon.alias.to_owned()).collect()
}

/// The alias the manifest enables on a fresh install.
#[must_use]
pub fn default_alias() -> String {
    APP_ICONS
        .iter()
        .find(|icon| icon.id == DEFAULT_ICON_ID)
        .map_or_else(String::new, |icon| icon.alias.to_owned())
}

/// # Errors
/// Returns a validation error for an id this build does not ship — the alias
/// behind it would not exist, and Android would refuse anyway.
pub fn resolve(id: &str) -> AppResult<AppIcon> {
    APP_ICONS
        .iter()
        .copied()
        .find(|icon| icon.id == id)
        .ok_or(AppError::Validation(ValidationError::Invalid {
            field: "app_icon",
        }))
}

/// The id behind an alias Android reports, falling back to the default.
#[must_use]
pub fn id_for_alias(alias: &str) -> &'static str {
    APP_ICONS
        .iter()
        .find(|icon| icon.alias == alias)
        .map_or(DEFAULT_ICON_ID, |icon| icon.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_one_of_the_icons_the_build_ships() {
        resolve(DEFAULT_ICON_ID).expect("the default has to exist");
    }

    #[test]
    fn ids_and_aliases_are_unique() {
        for icon in APP_ICONS {
            assert_eq!(
                APP_ICONS.iter().filter(|other| other.id == icon.id).count(),
                1,
                "two icons cannot share an id"
            );
            assert_eq!(
                APP_ICONS
                    .iter()
                    .filter(|other| other.alias == icon.alias)
                    .count(),
                1,
                "two icons cannot share an alias"
            );
        }
    }

    #[test]
    fn an_icon_this_build_does_not_ship_is_refused() {
        let error = resolve("holographic").expect_err("must refuse");
        assert_eq!(error.code(), "validation_invalid");
    }

    #[test]
    fn an_alias_android_does_not_recognise_reads_as_the_default() {
        assert_eq!(id_for_alias(""), DEFAULT_ICON_ID);
        assert_eq!(id_for_alias("Holographic"), DEFAULT_ICON_ID);
    }

    #[test]
    fn every_icon_carries_a_colour_settings_can_show() {
        for icon in APP_ICONS {
            assert!(
                icon.accent.starts_with('#') && icon.accent.len() == 7,
                "{} needs a six-digit hex accent",
                icon.id
            );
        }
    }
}

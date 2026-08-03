//! Choosing the icon the app wears.
//!
//! Android is the source of truth for which alias is enabled — the user can
//! have chosen one, been restored from a backup made on another phone, or
//! installed the app fresh. The stored setting is a note of the last choice,
//! used only when the platform has nothing to say.

use std::sync::Arc;

use crate::domain::app_icons::{
    aliases, app_icons, default_alias, id_for_alias, resolve, AppIcon, DEFAULT_ICON_ID,
    SELECTED_ICON_SETTING_KEY,
};
use crate::domain::settings::SettingsRepository;
use crate::error::AppResult;
use crate::platform::AppIconSwitch;

/// The catalogue plus whichever entry is showing now.
#[derive(Debug, Clone)]
pub struct AppIconCatalog {
    pub selected_id: String,
    pub items: Vec<AppIcon>,
}

pub struct AppIconUseCases {
    icons: Arc<dyn AppIconSwitch>,
    settings: Arc<dyn SettingsRepository>,
}

impl AppIconUseCases {
    #[must_use]
    pub fn new(icons: Arc<dyn AppIconSwitch>, settings: Arc<dyn SettingsRepository>) -> Self {
        Self { icons, settings }
    }

    /// # Errors
    /// Fails on a storage or platform error.
    pub fn catalog(&self) -> AppResult<AppIconCatalog> {
        let selected_id = match self.icons.current(&aliases(), &default_alias())? {
            Some(alias) => id_for_alias(&alias).to_owned(),
            // Off-device, or a platform that will not say: fall back to what
            // was last chosen, and to the manifest default before that.
            None => self
                .settings
                .read(SELECTED_ICON_SETTING_KEY)?
                .filter(|id| resolve(id).is_ok())
                .unwrap_or_else(|| DEFAULT_ICON_ID.to_owned()),
        };

        Ok(AppIconCatalog {
            selected_id,
            items: app_icons().to_vec(),
        })
    }

    /// Switches the launcher icon and remembers the choice.
    ///
    /// # Errors
    /// Fails for an icon this build does not ship, or when the platform refuses.
    pub fn select(&self, id: &str) -> AppResult<AppIconCatalog> {
        let icon = resolve(id)?;
        // Written down first, because switching closes the app so the home
        // screen redraws — anything after the platform call may never run.
        // A note that turns out to be wrong costs nothing: the catalogue is
        // read back from Android, which is the thing that actually decides.
        self.settings.write(SELECTED_ICON_SETTING_KEY, icon.id)?;
        self.icons
            .select(icon.alias, &aliases(), &default_alias())?;

        Ok(AppIconCatalog {
            selected_id: icon.id.to_owned(),
            items: app_icons().to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use parking_lot::Mutex;

    use crate::error::{AppError, PlatformError};

    use super::*;

    #[derive(Default)]
    struct FakeSwitch {
        current: Mutex<Option<String>>,
        refuse: bool,
    }

    impl AppIconSwitch for FakeSwitch {
        fn select(&self, alias: &str, _known: &[String], _fallback: &str) -> AppResult<()> {
            if self.refuse {
                return Err(AppError::Platform(PlatformError::PluginCall {
                    reason: "test".into(),
                }));
            }
            *self.current.lock() = Some(alias.to_owned());
            Ok(())
        }

        fn current(&self, _known: &[String], _fallback: &str) -> AppResult<Option<String>> {
            Ok(self.current.lock().clone())
        }
    }

    #[derive(Default)]
    struct FakeSettings {
        values: Mutex<Vec<(String, String)>>,
    }

    impl SettingsRepository for FakeSettings {
        fn read(&self, key: &str) -> AppResult<Option<String>> {
            Ok(self
                .values
                .lock()
                .iter()
                .find(|(stored, _)| stored == key)
                .map(|(_, value)| value.clone()))
        }

        fn write(&self, key: &str, value: &str) -> AppResult<()> {
            self.values.lock().push((key.to_owned(), value.to_owned()));
            Ok(())
        }
    }

    fn fixture(refuse: bool) -> (AppIconUseCases, Arc<FakeSwitch>, Arc<FakeSettings>) {
        let switch = Arc::new(FakeSwitch {
            refuse,
            ..FakeSwitch::default()
        });
        let settings = Arc::new(FakeSettings::default());
        (
            AppIconUseCases::new(
                Arc::clone(&switch) as Arc<dyn AppIconSwitch>,
                Arc::clone(&settings) as Arc<dyn SettingsRepository>,
            ),
            switch,
            settings,
        )
    }

    #[test]
    fn a_fresh_install_shows_the_icon_the_manifest_enabled() {
        let (icons, _switch, _settings) = fixture(false);
        assert_eq!(icons.catalog().expect("reads").selected_id, DEFAULT_ICON_ID);
    }

    #[test]
    fn choosing_an_icon_switches_it_and_writes_it_down() {
        let (icons, switch, settings) = fixture(false);

        let catalog = icons.select("neon").expect("selects");

        assert_eq!(catalog.selected_id, "neon");
        assert_eq!(*switch.current.lock(), Some("Neon".to_owned()));
        assert_eq!(
            settings.read(SELECTED_ICON_SETTING_KEY).expect("reads"),
            Some("neon".to_owned())
        );
    }

    #[test]
    fn what_the_platform_says_wins_over_what_was_written_down() {
        let (icons, switch, settings) = fixture(false);
        settings
            .write(SELECTED_ICON_SETTING_KEY, "amber")
            .expect("writes");
        *switch.current.lock() = Some("Paper".to_owned());

        assert_eq!(
            icons.catalog().expect("reads").selected_id,
            "paper",
            "the home screen is the truth, not the note we kept"
        );
    }

    #[test]
    fn a_refused_switch_leaves_the_home_screen_as_it_was() {
        let (icons, switch, _settings) = fixture(true);

        icons.select("neon").expect_err("must fail");

        assert_eq!(
            *switch.current.lock(),
            None,
            "the note we keep may be optimistic; the home screen must not be"
        );
    }

    #[test]
    fn an_icon_this_build_does_not_ship_never_reaches_the_platform() {
        let (icons, switch, _settings) = fixture(false);

        icons.select("holographic").expect_err("must refuse");

        assert_eq!(*switch.current.lock(), None);
    }
}

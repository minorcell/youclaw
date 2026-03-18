use rusqlite::OptionalExtension;

use super::*;

const MENU_BAR_ENABLED_KEY: &str = "desktop_menu_bar_enabled";

impl StorageService {
    pub fn get_menu_bar_enabled(&self) -> AppResult<bool> {
        self.get_bool_setting(MENU_BAR_ENABLED_KEY, false)
    }

    pub fn set_menu_bar_enabled(&self, enabled: bool) -> AppResult<()> {
        self.set_bool_setting(MENU_BAR_ENABLED_KEY, enabled)
    }

    fn get_bool_setting(&self, key: &str, default: bool) -> AppResult<bool> {
        let conn = self.open_connection()?;
        let value = conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get::<_, Option<String>>(0)
            })
            .optional()?
            .flatten();

        let Some(value) = value else {
            return Ok(default);
        };

        let normalized = value.trim();
        if normalized.eq_ignore_ascii_case("true") || normalized == "1" {
            return Ok(true);
        }
        if normalized.eq_ignore_ascii_case("false") || normalized == "0" {
            return Ok(false);
        }

        Ok(default)
    }

    fn set_bool_setting(&self, key: &str, value: bool) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![key, if value { "1" } else { "0" }],
        )?;
        Ok(())
    }
}

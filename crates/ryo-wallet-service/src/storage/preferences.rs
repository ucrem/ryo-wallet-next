use super::Theme;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub theme: Theme,
    pub idle_lock_seconds: u16,
    pub minimize_to_tray: bool,
    pub launch_on_startup: bool,
    pub notify_no_payment_id: bool,
    pub notify_weak_password: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            idle_lock_seconds: 300,
            minimize_to_tray: false,
            launch_on_startup: false,
            notify_no_payment_id: true,
            notify_weak_password: true,
        }
    }
}

impl Preferences {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.idle_lock_seconds != 0 && !(60..=3600).contains(&self.idle_lock_seconds) {
            return Err("idle timeout must be disabled or between one minute and one hour");
        }
        Ok(())
    }
}

pub fn idle_expired(last_activity_ms: u64, now_ms: u64, seconds: u16) -> bool {
    seconds != 0 && now_ms.saturating_sub(last_activity_ms) >= u64::from(seconds) * 1000
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_timer_covers_activity_suspend_disabled_and_clock_rollback() {
        assert!(!idle_expired(1000, 60999, 60));
        assert!(idle_expired(1000, 61000, 60));
        assert!(!idle_expired(60000, 61000, 60));
        assert!(idle_expired(1000, 900000, 60));
        assert!(!idle_expired(1000, u64::MAX, 0));
        assert!(!idle_expired(1000, 500, 60));
    }
    #[test]
    fn preferences_defaults_and_bounds_are_backward_compatible() {
        let mut prefs: Preferences = serde_json::from_str("{}").unwrap();
        assert_eq!(prefs, Preferences::default());
        prefs.idle_lock_seconds = 0;
        assert!(prefs.validate().is_ok());
        prefs.idle_lock_seconds = 59;
        assert!(prefs.validate().is_err());
        prefs.idle_lock_seconds = 3601;
        assert!(prefs.validate().is_err());
        assert!(serde_json::from_str::<Preferences>(r#"{"password":"secret"}"#).is_err());
    }
}

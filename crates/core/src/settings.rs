use jiff::civil::Weekday;

/// User-tunable rules. Stored as key/value pairs so new settings can be added
/// without a migration; unknown keys are ignored and missing keys use defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub office_target_min: u32,
    pub office_ceiling_min: u32,
    pub break_interval_min: u32,
    pub break_length_min: u32,
    pub distraction_grace_secs: u32,
    pub nudge_cooldown_secs: u32,
    pub week_start: Weekday,
    pub workdays: Vec<Weekday>,
    pub autostart: bool,
    pub appearance: Appearance,
    /// Start and end a meeting by itself while a Teams call holds the mic.
    pub detect_teams_calls: bool,
}

/// Light or dark window, or whichever Windows uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Appearance {
    #[default]
    Light,
    Dark,
    System,
}

impl Appearance {
    pub const ALL: [Self; 3] = [Self::Light, Self::Dark, Self::System];

    fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::System => "system",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.as_str() == s)
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum SettingsError {
    #[error("setting `{key}` has invalid value `{value}`")]
    InvalidValue { key: String, value: String },
    #[error("{0}")]
    Inconsistent(&'static str),
}

impl Default for Settings {
    fn default() -> Self {
        use Weekday::*;
        Self {
            office_target_min: 8 * 60,
            office_ceiling_min: 9 * 60,
            break_interval_min: 25,
            break_length_min: 5,
            distraction_grace_secs: 60,
            nudge_cooldown_secs: 120,
            week_start: Sunday,
            workdays: vec![Sunday, Monday, Tuesday, Wednesday, Thursday],
            autostart: true,
            appearance: Appearance::Light,
            detect_teams_calls: true,
        }
    }
}

const WEEKDAYS: [(Weekday, &str); 7] = [
    (Weekday::Sunday, "sun"),
    (Weekday::Monday, "mon"),
    (Weekday::Tuesday, "tue"),
    (Weekday::Wednesday, "wed"),
    (Weekday::Thursday, "thu"),
    (Weekday::Friday, "fri"),
    (Weekday::Saturday, "sat"),
];

fn weekday_str(day: Weekday) -> &'static str {
    WEEKDAYS
        .iter()
        .find(|(d, _)| *d == day)
        .map_or("sun", |(_, s)| s)
}

fn parse_weekday(s: &str) -> Option<Weekday> {
    WEEKDAYS.iter().find(|(_, n)| *n == s).map(|(d, _)| *d)
}

impl Settings {
    pub fn to_pairs(&self) -> Vec<(&'static str, String)> {
        let workdays: Vec<_> = self.workdays.iter().map(|d| weekday_str(*d)).collect();
        vec![
            ("office_target_min", self.office_target_min.to_string()),
            ("office_ceiling_min", self.office_ceiling_min.to_string()),
            ("break_interval_min", self.break_interval_min.to_string()),
            ("break_length_min", self.break_length_min.to_string()),
            (
                "distraction_grace_secs",
                self.distraction_grace_secs.to_string(),
            ),
            ("nudge_cooldown_secs", self.nudge_cooldown_secs.to_string()),
            ("week_start", weekday_str(self.week_start).to_owned()),
            ("workdays", workdays.join(",")),
            ("autostart", self.autostart.to_string()),
            ("appearance", self.appearance.as_str().to_owned()),
            ("detect_teams_calls", self.detect_teams_calls.to_string()),
        ]
    }

    /// Builds settings from stored pairs. Missing keys keep their defaults and
    /// unknown keys are ignored, so older and newer databases both load.
    pub fn from_pairs<K, V>(pairs: impl IntoIterator<Item = (K, V)>) -> Result<Self, SettingsError>
    where
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let mut s = Self::default();
        for (key, value) in pairs {
            let (key, value) = (key.as_ref(), value.as_ref());
            let invalid = || SettingsError::InvalidValue {
                key: key.to_owned(),
                value: value.to_owned(),
            };
            let num = || value.parse::<u32>().map_err(|_| invalid());
            match key {
                "office_target_min" => s.office_target_min = num()?,
                "office_ceiling_min" => s.office_ceiling_min = num()?,
                "break_interval_min" => s.break_interval_min = num()?,
                "break_length_min" => s.break_length_min = num()?,
                "distraction_grace_secs" => s.distraction_grace_secs = num()?,
                "nudge_cooldown_secs" => s.nudge_cooldown_secs = num()?,
                "week_start" => s.week_start = parse_weekday(value).ok_or_else(invalid)?,
                "workdays" => {
                    s.workdays = value
                        .split(',')
                        .filter(|d| !d.is_empty())
                        .map(|d| parse_weekday(d).ok_or_else(invalid))
                        .collect::<Result<_, _>>()?;
                }
                "autostart" => s.autostart = value.parse().map_err(|_| invalid())?,
                "appearance" => s.appearance = Appearance::parse(value).ok_or_else(invalid)?,
                "detect_teams_calls" => {
                    s.detect_teams_calls = value.parse().map_err(|_| invalid())?;
                }
                _ => {}
            }
        }
        s.validate()?;
        Ok(s)
    }

    pub fn validate(&self) -> Result<(), SettingsError> {
        if self.office_ceiling_min < self.office_target_min {
            return Err(SettingsError::Inconsistent(
                "office soft ceiling must be at least the office target",
            ));
        }
        if self.break_interval_min == 0 {
            return Err(SettingsError::Inconsistent(
                "break interval must be at least 1 minute",
            ));
        }
        Ok(())
    }
}

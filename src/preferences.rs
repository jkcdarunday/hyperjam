use crate::speed::ScrollSpeed;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub speed: ScrollSpeed,
    pub offset: f64,
    pub volume: f32,
    pub difficulty: usize,
    pub autoplay: bool,
    pub fullscreen: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            speed: ScrollSpeed::default(),
            offset: 0.0,
            volume: 0.7,
            difficulty: 1,
            autoplay: false,
            fullscreen: false,
        }
    }
}

impl Preferences {
    pub fn load(path: &Path) -> (Self, Vec<String>) {
        let read = (|| {
            let file = std::fs::File::open(path)?;
            let mut text = String::new();
            file.take(16_385).read_to_string(&mut text)?;
            if text.len() > 16_384 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "settings file exceeds 16 KiB",
                ));
            }
            Ok(text)
        })();
        match read {
            Ok(text) => Self::parse(&text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (Self::default(), vec![]),
            Err(error) => (
                Self::default(),
                vec![format!("Could not read {}: {error}", path.display())],
            ),
        }
    }

    fn parse(text: &str) -> (Self, Vec<String>) {
        let mut prefs = Self::default();
        let mut warnings = vec![];
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                warnings.push(format!("Settings line {}: expected key=value", index + 1));
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            let result: Result<(), String> = match key {
                "speed" => ScrollSpeed::parse(value).map(|v| prefs.speed = v),
                "offset_ms" => {
                    parse_number(value, -500.0, 500.0).map(|v| prefs.offset = v / 1000.0)
                }
                "volume_percent" => {
                    parse_number(value, 0.0, 100.0).map(|v| prefs.volume = v as f32 / 100.0)
                }
                "difficulty" => match value {
                    "easy" => {
                        prefs.difficulty = 0;
                        Ok(())
                    }
                    "normal" => {
                        prefs.difficulty = 1;
                        Ok(())
                    }
                    "hard" => {
                        prefs.difficulty = 2;
                        Ok(())
                    }
                    _ => Err("expected easy, normal, or hard".into()),
                },
                "autoplay" => value
                    .parse()
                    .map(|v| prefs.autoplay = v)
                    .map_err(|_| "expected true or false".into()),
                "fullscreen" => value
                    .parse()
                    .map(|v| prefs.fullscreen = v)
                    .map_err(|_| "expected true or false".into()),
                _ => Ok(()), // Allow newer versions to add preferences.
            };
            if let Err(error) = result {
                warnings.push(format!("Settings line {} ({key}): {error}", index + 1));
            }
        }
        (prefs, warnings)
    }

    pub fn save(self, path: &Path) -> Result<(), String> {
        if self.difficulty > 2
            || !self.offset.is_finite()
            || self.offset.abs() > 0.5
            || !self.volume.is_finite()
            || !(0.0..=1.0).contains(&self.volume)
        {
            return Err("Invalid preference value".into());
        }
        let text = format!(
            "# Hyperjam preferences\nspeed={}\noffset_ms={}\nvolume_percent={}\ndifficulty={}\nautoplay={}\nfullscreen={}\n",
            self.speed.multiplier(),
            self.offset * 1000.0,
            self.volume * 100.0,
            ["easy", "normal", "hard"][self.difficulty],
            self.autoplay,
            self.fullscreen
        );
        let temp = path.with_extension("settings.tmp");
        let write = (|| {
            let mut file = std::fs::File::create(&temp)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            std::fs::rename(&temp, path)
        })();
        write.map_err(|error| format!("Could not save {}: {error}", path.display()))
    }
}

fn parse_number(value: &str, min: f64, max: f64) -> Result<f64, String> {
    let number = value
        .parse::<f64>()
        .map_err(|_| "expected a number".to_owned())?;
    if number.is_finite() && (min..=max).contains(&number) {
        Ok(number)
    } else {
        Err(format!("expected a finite number between {min} and {max}"))
    }
}

/// Save once controls settle, instead of writing on every frame of a slider drag.
pub struct PreferenceStore {
    path: PathBuf,
    saved: Preferences,
    observed: Preferences,
    changed_at: Option<f64>,
}
impl PreferenceStore {
    pub fn new(path: PathBuf, saved: Preferences) -> Self {
        Self {
            path,
            saved,
            observed: saved,
            changed_at: None,
        }
    }
    pub fn update(&mut self, current: Preferences, now: f64) -> Result<bool, String> {
        if current != self.observed {
            self.observed = current;
            self.changed_at = Some(now);
        }
        if self.observed == self.saved {
            self.changed_at = None;
        }
        if self.changed_at.is_some_and(|time| now - time >= 0.3) {
            self.changed_at = None;
            self.write()?;
            return Ok(true);
        }
        Ok(false)
    }
    pub fn flush(&mut self, current: Preferences) -> Result<(), String> {
        self.observed = current;
        self.changed_at = None;
        if self.observed != self.saved {
            self.write()?;
        }
        Ok(())
    }
    fn write(&mut self) -> Result<(), String> {
        self.observed.save(&self.path)?;
        self.saved = self.observed;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "hyperjam-preferences-{}-{name}",
            std::process::id()
        ))
    }
    #[test]
    fn saved_values_round_trip() {
        let path = path("roundtrip.settings");
        let prefs = Preferences {
            speed: ScrollSpeed::parse("8").unwrap(),
            offset: -0.025,
            volume: 0.35,
            difficulty: 2,
            autoplay: true,
            fullscreen: true,
        };
        prefs.save(&path).unwrap();
        let (loaded, warnings) = Preferences::load(&path);
        assert_eq!(loaded, prefs);
        assert!(warnings.is_empty());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn malformed_fields_do_not_discard_valid_preferences() {
        let (prefs, warnings) = Preferences::parse(
            "speed=6\noffset_ms=NaN\nvolume_percent=120\ndifficulty=oops\nautoplay=maybe\nfullscreen=true\nfuture_option=anything\n",
        );
        assert_eq!(prefs.speed.multiplier(), 6.0);
        assert!(prefs.fullscreen);
        assert_eq!(prefs.offset, 0.0);
        assert_eq!(prefs.volume, 0.7);
        assert_eq!(prefs.difficulty, 1);
        assert!(!prefs.autoplay);
        assert_eq!(warnings.len(), 4);
    }
    #[test]
    fn missing_file_uses_defaults() {
        let (prefs, warnings) = Preferences::load(&path("missing.settings"));
        assert_eq!(prefs, Preferences::default());
        assert!(warnings.is_empty());
    }
    #[test]
    fn changes_debounce_and_flush_keeps_last_adjustment() {
        let path = path("debounce.settings");
        let defaults = Preferences::default();
        let mut store = PreferenceStore::new(path.clone(), defaults);
        let mut current = defaults;
        current.speed = current.speed.next();
        store.update(current, 0.0).unwrap();
        assert!(!path.exists());
        current.speed = current.speed.next();
        store.update(current, 0.2).unwrap();
        store.update(current, 0.4).unwrap();
        assert!(!path.exists());
        store.update(current, 0.6).unwrap();
        assert_eq!(Preferences::load(&path).0, current);
        current.offset = 0.01;
        store.update(current, 0.7).unwrap();
        store.flush(current).unwrap();
        assert_eq!(Preferences::load(&path).0, current);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn failed_save_does_not_retry_every_frame() {
        let mut store = PreferenceStore::new(
            path("absent-parent").join("prefs.settings"),
            Preferences::default(),
        );
        let prefs = Preferences {
            autoplay: true,
            ..Preferences::default()
        };
        store.update(prefs, 0.0).unwrap();
        assert!(store.update(prefs, 0.4).is_err());
        assert!(store.update(prefs, 0.5).is_ok());
        assert!(store.flush(prefs).is_err());
    }
}

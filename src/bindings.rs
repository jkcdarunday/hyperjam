use macroquad::prelude::KeyCode;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bindings {
    pub keys: [KeyCode; 7],
}

impl Default for Bindings {
    fn default() -> Self {
        Self {
            keys: [
                KeyCode::S,
                KeyCode::D,
                KeyCode::F,
                KeyCode::Space,
                KeyCode::J,
                KeyCode::K,
                KeyCode::L,
            ],
        }
    }
}

// Keep control keys separate. Letters used by legacy shortcuts (R/O) are allowed;
// the app disables those shortcuts when that key belongs to a note lane.
pub fn allowed_keys() -> Vec<KeyCode> {
    use KeyCode::*;
    vec![
        A,
        B,
        C,
        D,
        E,
        F,
        G,
        H,
        I,
        J,
        K,
        L,
        M,
        N,
        O,
        P,
        Q,
        R,
        S,
        T,
        U,
        V,
        W,
        X,
        Y,
        Z,
        Key0,
        Key1,
        Key2,
        Key3,
        Key4,
        Key5,
        Key6,
        Key7,
        Key8,
        Key9,
        Space,
        LeftShift,
        RightShift,
        LeftControl,
        RightControl,
        LeftAlt,
        RightAlt,
        Left,
        Right,
        Comma,
        Period,
        Slash,
        Semicolon,
        Apostrophe,
        LeftBracket,
        RightBracket,
        Backslash,
        Kp0,
        Kp1,
        Kp2,
        Kp3,
        Kp4,
        Kp5,
        Kp6,
        Kp7,
        Kp8,
        Kp9,
    ]
}

pub fn key_name(key: KeyCode) -> String {
    use KeyCode::*;
    match key {
        Space => "SPACE".into(),
        LeftShift => "LSHIFT".into(),
        RightShift => "RSHIFT".into(),
        LeftControl => "LCTRL".into(),
        RightControl => "RCTRL".into(),
        LeftAlt => "LALT".into(),
        RightAlt => "RALT".into(),
        Key0 | Key1 | Key2 | Key3 | Key4 | Key5 | Key6 | Key7 | Key8 | Key9 => {
            format!("{key:?}").trim_start_matches("Key").to_owned()
        }
        Comma => ",".into(),
        Period => ".".into(),
        Slash => "/".into(),
        Semicolon => ";".into(),
        Apostrophe => "'".into(),
        LeftBracket => "[".into(),
        RightBracket => "]".into(),
        Backslash => "\\".into(),
        _ => format!("{key:?}").to_uppercase(),
    }
}

impl Bindings {
    pub fn parse(text: &str) -> Result<Self, String> {
        let names: Vec<_> = text.split_whitespace().collect();
        if names.len() != 7 {
            return Err("Choose exactly seven keys, separated by spaces.".into());
        }
        let allowed = allowed_keys();
        let mut keys = [KeyCode::Space; 7];
        for (lane, name) in names.iter().enumerate() {
            keys[lane] = *allowed
                .iter()
                .find(|&&key| {
                    key_name(key).eq_ignore_ascii_case(name)
                        || format!("{key:?}").eq_ignore_ascii_case(name)
                })
                .ok_or_else(|| format!("Unsupported or reserved key: {name}"))?;
        }
        let bindings = Self { keys };
        bindings.validate()?;
        Ok(bindings)
    }
    pub fn validate(&self) -> Result<(), String> {
        let allowed = allowed_keys();
        for (lane, key) in self.keys.iter().enumerate() {
            if !allowed.contains(key) {
                return Err(format!("{} is reserved for game controls.", key_name(*key)));
            }
            if self.keys[..lane].contains(key) {
                return Err(format!(
                    "{} is assigned to more than one lane.",
                    key_name(*key)
                ));
            }
        }
        Ok(())
    }
    pub fn labels(&self) -> [String; 7] {
        self.keys.map(key_name)
    }
    pub fn text(&self) -> String {
        self.labels().join(" ")
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let temp = path.with_extension("keys.tmp");
        std::fs::write(&temp, format!("{}\n", self.text()))
            .map_err(|e| format!("Could not save keys: {e}"))?;
        std::fs::rename(&temp, path).map_err(|e| format!("Could not save keys: {e}"))
    }
    /// Swapping avoids duplicate bindings while remapping an existing key.
    pub fn assign(&mut self, lane: usize, key: KeyCode) -> Result<(), String> {
        if lane >= 7 {
            return Err("Invalid lane".into());
        }
        if !allowed_keys().contains(&key) {
            return Err(
                "That key is reserved. Choose a letter, number, modifier, or punctuation key."
                    .into(),
            );
        }
        if let Some(other) = self.keys.iter().position(|&k| k == key) {
            self.keys.swap(lane, other);
        } else {
            self.keys[lane] = key;
        }
        self.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_round_trips_every_supported_key() {
        for key in allowed_keys() {
            let mut bindings = Bindings::default();
            bindings.assign(0, key).unwrap();
            assert_eq!(Bindings::parse(&bindings.text()).unwrap(), bindings);
        }
    }
    #[test]
    fn duplicate_and_reserved_keys_are_rejected() {
        assert!(Bindings::parse("S D F SPACE J K K").is_err());
        assert!(Bindings::parse("S D F ENTER J K L").is_err());
        assert!(Bindings::parse("A B C").is_err());
    }
    #[test]
    fn choosing_an_existing_key_swaps_lanes() {
        let mut bindings = Bindings::default();
        bindings.assign(0, KeyCode::J).unwrap();
        assert_eq!(bindings.keys[0], KeyCode::J);
        assert_eq!(bindings.keys[4], KeyCode::S);
    }
    #[test]
    fn bindings_persist_to_disk() {
        let path =
            std::env::temp_dir().join(format!("hyperjam-bindings-{}.keys", std::process::id()));
        let bindings = Bindings::parse("Z X C SPACE B N M").unwrap();
        bindings.save(&path).unwrap();
        assert_eq!(Bindings::load(&path).unwrap(), bindings);
        std::fs::remove_file(path).unwrap();
    }
}

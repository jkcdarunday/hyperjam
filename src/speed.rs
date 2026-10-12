//! Available scroll-speed multiplier presets.

pub const PRESETS: [f32; 14] = [
    0.2, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 6.0, 8.0, 99.0,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrollSpeed(usize);

impl Default for ScrollSpeed {
    fn default() -> Self {
        Self(2)
    }
}

impl ScrollSpeed {
    pub fn from_index(index: usize) -> Self {
        Self(index.min(PRESETS.len() - 1))
    }
    pub fn multiplier(self) -> f32 {
        PRESETS[self.0]
    }
    pub fn previous(self) -> Self {
        Self(self.0.saturating_sub(1))
    }
    pub fn next(self) -> Self {
        Self::from_index(self.0 + 1)
    }
    pub fn label(self) -> String {
        format!("{:.1}x", self.multiplier())
    }
    pub fn parse(value: &str) -> Result<Self, String> {
        let value = value
            .trim()
            .trim_end_matches(['x', 'X'])
            .parse::<f32>()
            .map_err(|_| "Invalid speed multiplier")?;
        PRESETS
            .iter()
            .position(|&preset| preset == value)
            .map(Self)
            .ok_or_else(|| {
                format!(
                    "Choose a speed preset: {}",
                    PRESETS
                        .iter()
                        .map(|speed| format!("{speed:.1}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controls_visit_every_preset_in_order() {
        let mut speed = ScrollSpeed::from_index(0);
        for value in PRESETS {
            assert_eq!(speed.multiplier(), value);
            speed = speed.next();
        }
        assert_eq!(speed.multiplier(), 99.0);
        for value in PRESETS.iter().rev() {
            assert_eq!(speed.multiplier(), *value);
            speed = speed.previous();
        }
        assert_eq!(speed.multiplier(), 0.2);
    }
    #[test]
    fn cli_accepts_exact_presets_and_rejects_other_values() {
        for value in PRESETS {
            let speed = ScrollSpeed::parse(&value.to_string()).unwrap();
            assert_eq!(ScrollSpeed::parse(&speed.label()).unwrap(), speed);
        }
        for invalid in [
            "0", "0.1", "0.3", "3.2", "5.5", "7", "98", "100", "NaN", "inf", "oops",
        ] {
            assert!(ScrollSpeed::parse(invalid).is_err(), "accepted {invalid}");
        }
    }
}

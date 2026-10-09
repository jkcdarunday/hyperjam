use crate::chart::{Chart, SoundEvent};

pub const MISS_WINDOW: f64 = 0.160;
pub const RELEASE_WINDOW: f64 = 0.100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Judgment {
    Perfect,
    Cool,
    Good,
    Miss,
}
impl Judgment {
    pub fn from_error(error: f64) -> Self {
        match error.abs() {
            e if e <= 0.035 => Self::Perfect,
            e if e <= 0.070 => Self::Cool,
            e if e <= MISS_WINDOW => Self::Good,
            _ => Self::Miss,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Perfect => "PERFECT",
            Self::Cool => "COOL",
            Self::Good => "GOOD",
            Self::Miss => "MISS",
        }
    }
    pub fn index(self) -> usize {
        match self {
            Self::Perfect => 0,
            Self::Cool => 1,
            Self::Good => 2,
            Self::Miss => 3,
        }
    }
    pub fn weight(self) -> f64 {
        match self {
            Self::Perfect => 1.0,
            Self::Cool => 0.8,
            Self::Good => 0.5,
            Self::Miss => 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteState {
    Pending,
    Holding,
    Done,
}

pub struct Gameplay {
    pub chart: Chart,
    pub states: Vec<NoteState>,
    pub counts: [usize; 4],
    pub combo: usize,
    pub max_combo: usize,
    pub score: u64,
    pub health: f32,
    pub last: Option<(Judgment, f64, f64)>,
    pub pressed: [bool; 7],
    pub flashes: [f64; 7],
    pub autoplay: bool,
    holding: [Option<usize>; 7],
    next: [usize; 7],
    lanes: [Vec<usize>; 7],
}

impl Gameplay {
    pub fn new(chart: Chart, autoplay: bool) -> Self {
        let states = vec![NoteState::Pending; chart.notes.len()];
        let lanes = std::array::from_fn(|lane| {
            chart
                .notes
                .iter()
                .enumerate()
                .filter_map(|(i, n)| (n.lane == lane).then_some(i))
                .collect()
        });
        Self {
            chart,
            states,
            counts: [0; 4],
            combo: 0,
            max_combo: 0,
            score: 0,
            health: 1.0,
            last: None,
            pressed: [false; 7],
            flashes: [-100.0; 7],
            autoplay,
            holding: [None; 7],
            next: [0; 7],
            lanes,
        }
    }
    fn judge(&mut self, judgment: Judgment, time: f64, error: f64) {
        self.counts[judgment.index()] += 1;
        if judgment == Judgment::Miss {
            self.combo = 0;
            self.health = (self.health - 0.06).max(0.0);
        } else {
            self.combo += 1;
            self.max_combo = self.max_combo.max(self.combo);
            self.score += (judgment.weight() * 1000.0) as u64;
            self.health = (self.health + 0.012).min(1.0);
        }
        self.last = Some((judgment, time, error));
    }
    fn pending(&mut self, lane: usize) -> Option<usize> {
        while let Some(&i) = self.lanes[lane].get(self.next[lane]) {
            if self.states[i] == NoteState::Pending {
                return Some(i);
            }
            self.next[lane] += 1;
        }
        None
    }
    pub fn press(&mut self, lane: usize, time: f64) -> Option<SoundEvent> {
        self.pressed[lane] = true;
        let i = self.pending(lane)?;
        let error = time - self.chart.notes[i].sound.time;
        if error.abs() > MISS_WINDOW {
            return None;
        }
        let sound = self.chart.notes[i].sound.clone();
        self.judge(Judgment::from_error(error), time, error);
        self.flashes[lane] = time;
        if self.chart.notes[i].end.is_some() {
            self.states[i] = NoteState::Holding;
            self.holding[lane] = Some(i);
        } else {
            self.states[i] = NoteState::Done;
        }
        Some(sound)
    }
    pub fn release(&mut self, lane: usize, time: f64) {
        self.pressed[lane] = false;
        if let Some(i) = self.holding[lane].take() {
            let error = time - self.chart.notes[i].end.unwrap();
            self.states[i] = NoteState::Done;
            self.judge(
                if error < -RELEASE_WINDOW {
                    Judgment::Miss
                } else {
                    Judgment::Perfect
                },
                time,
                error,
            );
        }
    }
    pub fn update(&mut self, time: f64) {
        for lane in 0..7 {
            if self.autoplay {
                while let Some(i) = self.pending(lane) {
                    let head = self.chart.notes[i].sound.time;
                    if head > time {
                        break;
                    }
                    self.press(lane, head);
                    if self.chart.notes[i].end.is_none() {
                        self.pressed[lane] = false;
                    }
                }
            }
            while let Some(i) = self.pending(lane) {
                if time - self.chart.notes[i].sound.time <= MISS_WINDOW {
                    break;
                }
                self.states[i] = NoteState::Done;
                self.judge(Judgment::Miss, time, 0.0);
                if self.chart.notes[i].end.is_some() {
                    self.judge(Judgment::Miss, time, 0.0);
                }
            }
            if let Some(i) = self.holding[lane] {
                let end = self.chart.notes[i].end.unwrap();
                if time >= end {
                    self.release(lane, end);
                    self.flashes[lane] = time;
                }
            }
        }
    }
    pub fn accuracy(&self) -> f64 {
        let total: usize = self.counts.iter().sum();
        if total == 0 {
            return 100.0;
        }
        (self.counts[0] as f64 + self.counts[1] as f64 * 0.8 + self.counts[2] as f64 * 0.5)
            / total as f64
            * 100.0
    }
    pub fn finished(&self, time: f64) -> bool {
        time > self.chart.duration && self.states.iter().all(|s| *s == NoteState::Done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::Note;
    fn chart() -> Chart {
        Chart {
            warnings: Vec::new(),
            level: 1,
            notes: vec![Note {
                lane: 0,
                sound: SoundEvent {
                    time: 1.0,
                    sample: 0,
                    volume: 1.0,
                    pan: 0.0,
                },
                end: Some(2.0),
            }],
            background: vec![],
            duration: 3.0,
        }
    }
    #[test]
    fn early_release_breaks_combo() {
        let mut g = Gameplay::new(chart(), false);
        assert!(g.press(0, 1.0).is_some());
        g.release(0, 1.5);
        assert_eq!(g.counts, [1, 0, 0, 1]);
        assert_eq!(g.combo, 0);
    }
    #[test]
    fn holding_to_end_scores_tail() {
        let mut g = Gameplay::new(chart(), false);
        g.press(0, 1.02);
        g.update(2.01);
        assert_eq!(g.counts, [2, 0, 0, 0]);
        assert_eq!(g.max_combo, 2);
    }
    #[test]
    fn missed_hold_counts_head_and_tail() {
        let mut g = Gameplay::new(chart(), false);
        g.update(1.2);
        assert_eq!(g.counts, [0, 0, 0, 2]);
        assert!(g.press(0, 1.3).is_none());
    }
    #[test]
    fn autoplay_catches_up_after_frame_stall() {
        let mut g = Gameplay::new(chart(), true);
        g.update(2.5);
        assert_eq!(g.counts, [2, 0, 0, 0]);
        assert_eq!(g.states, [NoteState::Done]);
    }
    #[test]
    fn judgment_edges() {
        assert_eq!(Judgment::from_error(-0.035), Judgment::Perfect);
        assert_eq!(Judgment::from_error(0.070), Judgment::Cool);
        assert_eq!(Judgment::from_error(0.160), Judgment::Good);
        assert_eq!(Judgment::from_error(0.161), Judgment::Miss);
    }
}

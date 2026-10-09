use crate::{
    chart::{Song, SoundEvent, parse_ojn},
    ojm::parse_ojm,
};
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Source};
use std::{
    collections::HashMap,
    io::Cursor,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    time::Duration,
};

pub type Bank = HashMap<u16, Arc<[f32]>>; // Interleaved stereo at the output device's sample rate.
pub struct LoadedSong {
    pub song: Song,
    pub bank: Bank,
    pub warnings: Vec<String>,
}

fn read_file(path: &Path) -> Result<Vec<u8>, String> {
    let size = std::fs::metadata(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .len();
    if size > 512 * 1024 * 1024 {
        return Err("File exceeds the 512 MiB safety limit".into());
    }
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn load_song(
    path: &Path,
    sample_override: Option<&Path>,
    rate: u32,
) -> Result<LoadedSong, String> {
    if rate == 0 {
        return Err("Audio sample rate must be positive".into());
    }
    let mut song = parse_ojn(&read_file(path)?)?;
    let mut bank = Bank::new();
    let mut warnings = Vec::new();
    for (difficulty, chart) in ["Easy", "Normal", "Hard"].iter().zip(&song.charts) {
        if !chart.warnings.is_empty() {
            warnings.push(format!(
                "{difficulty}: {} long-note authoring issues normalized. {}",
                chart.warnings.len(),
                chart.warnings[0]
            ));
        }
    }
    let sample = sample_override
        .map(Path::to_path_buf)
        .or_else(|| find_samples(path, &song.sample_file));
    if let Some(sample) = sample {
        let encoded = parse_ojm(&read_file(&sample)?)?;
        let mut total = 0;
        for (id, bytes) in encoded {
            match decode(bytes, rate) {
                Ok(pcm) => {
                    total += pcm.len() * 4;
                    if total > 512 * 1024 * 1024 {
                        return Err("Decoded audio exceeds the 512 MiB safety limit".into());
                    }
                    bank.insert(id, pcm.into());
                }
                Err(e) => warnings.push(format!("Sample {id}: {e}")),
            }
        }
    } else {
        warnings.push(format!("Missing keysounds: {}. Put the sample bank beside the OJN, or drop it onto the window.", song.sample_file));
    }
    let missing: std::collections::HashSet<u16> = song
        .charts
        .iter()
        .flat_map(|c| {
            c.notes
                .iter()
                .map(|n| n.sound.sample)
                .chain(c.background.iter().map(|s| s.sample))
        })
        .filter(|id| !bank.contains_key(id))
        .collect();
    if !missing.is_empty() {
        warnings.push(format!(
            "{} referenced samples are unavailable; those events will be silent.",
            missing.len()
        ));
    }
    // Backing samples can contain an entire song; let their audible tails finish.
    for chart in &mut song.charts {
        for event in chart
            .background
            .iter()
            .chain(chart.notes.iter().map(|n| &n.sound))
        {
            if let Some(pcm) = bank.get(&event.sample) {
                chart.duration = chart
                    .duration
                    .max(event.time + pcm.len() as f64 / (2.0 * rate as f64) + 0.3);
            }
        }
    }
    Ok(LoadedSong {
        song,
        bank,
        warnings,
    })
}

fn find_samples(chart: &Path, filename: &str) -> Option<PathBuf> {
    let folder = chart.parent().unwrap_or(Path::new("."));
    let filename = filename.rsplit(['/', '\\']).next()?;
    if filename.is_empty() {
        return None;
    }
    let path = folder.join(filename);
    if path.is_file() {
        return Some(path);
    }
    std::fs::read_dir(folder)
        .ok()?
        .filter_map(Result::ok)
        .find(|f| {
            f.file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(filename)
        })
        .map(|f| f.path())
}

fn decode(bytes: Vec<u8>, rate: u32) -> Result<Vec<f32>, String> {
    let decoder = Decoder::try_from(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let channels = decoder.channels() as usize;
    let original_rate = decoder.sample_rate();
    if channels == 0 || original_rate == 0 {
        return Err("Invalid audio channel count/sample rate".into());
    }
    let mut original = Vec::new();
    for value in decoder {
        if original.len() >= 32 * 1024 * 1024 {
            return Err("Decoded sample is too large".into());
        }
        original.push(if value.is_finite() { value } else { 0.0 });
    }
    let frames = original.len() / channels;
    if frames == 0 {
        return Ok(vec![]);
    }
    let out_frames = (frames as u64 * rate as u64 / original_rate as u64) as usize;
    if out_frames > 16 * 1024 * 1024 {
        return Err("Resampled audio is too large".into());
    }
    let mut pcm = Vec::with_capacity(out_frames * 2);
    for frame in 0..out_frames {
        let position = frame as f64 * original_rate as f64 / rate as f64;
        let a = (position.floor() as usize).min(frames - 1);
        let b = (a + 1).min(frames - 1);
        let fraction = position.fract() as f32;
        for channel in 0..2 {
            let channel = channel.min(channels - 1);
            pcm.push(
                original[a * channels + channel] * (1.0 - fraction)
                    + original[b * channels + channel] * fraction,
            );
        }
    }
    Ok(pcm)
}

pub fn practice_bank(rate: u32) -> Bank {
    let mut bank = Bank::new();
    for id in 0..9u16 {
        let frequency = [
            261.63, 293.66, 329.63, 392.0, 440.0, 523.25, 587.33, 60.0, 1800.0,
        ][id as usize];
        let duration = if id < 7 { 0.5 } else { 0.14 };
        let pcm: Vec<f32> = (0..(rate as f32 * duration) as usize)
            .flat_map(|i| {
                let t = i as f32 / rate as f32;
                let envelope = (t * 300.0).min(1.0) * (-t * if id < 7 { 9.0 } else { 35.0 }).exp();
                let phase = std::f32::consts::TAU
                    * if id == 7 {
                        frequency * t + 35.0 * (1.0 - (-t * 25.0).exp()) / 25.0
                    } else {
                        frequency * t
                    };
                let value = phase.sin() * envelope * 0.25;
                [value, value]
            })
            .collect();
        bank.insert(id, pcm.into());
    }
    bank
}

enum Command {
    Start(Bank, Vec<SoundEvent>),
    Hit(SoundEvent),
    Stop,
}
struct Shared {
    frame: AtomicI64,
    running: AtomicBool,
    paused: AtomicBool,
    volume: AtomicU32,
}
impl Shared {
    fn new() -> Self {
        Self {
            frame: AtomicI64::new(0),
            running: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            volume: AtomicU32::new(0.7f32.to_bits()),
        }
    }
}

pub struct Audio {
    _stream: OutputStream,
    sender: Sender<Command>,
    shared: Arc<Shared>,
    pub rate: u32,
}
impl Audio {
    pub fn open() -> Result<Self, String> {
        let mut stream = OutputStreamBuilder::open_default_stream().map_err(|e| e.to_string())?;
        stream.log_on_drop(false);
        let rate = stream.config().sample_rate();
        let shared = Arc::new(Shared::new());
        let (sender, receiver) = mpsc::channel();
        stream
            .mixer()
            .add(Mixer::new(rate, shared.clone(), receiver));
        Ok(Self {
            _stream: stream,
            sender,
            shared,
            rate,
        })
    }
    pub fn start(&self, bank: Bank, events: Vec<SoundEvent>) {
        self.shared.paused.store(false, Ordering::Relaxed);
        self.shared
            .frame
            .store(-2 * self.rate as i64, Ordering::Relaxed);
        let _ = self.sender.send(Command::Start(bank, events));
    }
    pub fn hit(&self, sound: SoundEvent) {
        let _ = self.sender.send(Command::Hit(sound));
    }
    pub fn stop(&self) {
        let _ = self.sender.send(Command::Stop);
    }
    pub fn pause(&self, paused: bool) {
        self.shared.paused.store(paused, Ordering::Relaxed);
    }
    pub fn time(&self) -> f64 {
        self.shared.frame.load(Ordering::Relaxed) as f64 / self.rate as f64
    }
    pub fn volume(&self, volume: f32) {
        self.shared
            .volume
            .store(volume.to_bits(), Ordering::Relaxed);
    }
}

struct Voice {
    pcm: Arc<[f32]>,
    position: usize,
    gain: [f32; 2],
}
struct Mixer {
    rate: u32,
    shared: Arc<Shared>,
    receiver: Receiver<Command>,
    bank: Bank,
    events: Vec<SoundEvent>,
    next_event: usize,
    voices: Vec<Voice>,
    frame: i64,
    right: Option<f32>,
}
impl Mixer {
    fn new(rate: u32, shared: Arc<Shared>, receiver: Receiver<Command>) -> Self {
        Self {
            rate,
            shared,
            receiver,
            bank: Bank::new(),
            events: vec![],
            next_event: 0,
            voices: Vec::with_capacity(256),
            frame: 0,
            right: None,
        }
    }
    fn voice(&mut self, sound: SoundEvent) {
        if let Some(pcm) = self.bank.get(&sound.sample) {
            if self.voices.len() >= 256 {
                self.voices.remove(0);
            }
            self.voices.push(Voice {
                pcm: pcm.clone(),
                position: 0,
                gain: [
                    sound.volume * (1.0 - sound.pan.max(0.0)),
                    sound.volume * (1.0 + sound.pan.min(0.0)),
                ],
            });
        }
    }
}
impl Iterator for Mixer {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if let Some(right) = self.right.take() {
            return Some(right);
        }
        while let Ok(command) = self.receiver.try_recv() {
            match command {
                Command::Start(bank, mut events) => {
                    events.sort_by(|a, b| a.time.total_cmp(&b.time));
                    self.bank = bank;
                    self.events = events;
                    self.next_event = 0;
                    self.voices.clear();
                    self.frame = -2 * self.rate as i64;
                    self.shared.running.store(true, Ordering::Relaxed);
                }
                Command::Hit(sound) => self.voice(sound),
                Command::Stop => {
                    self.voices.clear();
                    self.events.clear();
                    self.shared.running.store(false, Ordering::Relaxed);
                }
            }
        }
        if !self.shared.running.load(Ordering::Relaxed)
            || self.shared.paused.load(Ordering::Relaxed)
        {
            self.right = Some(0.0);
            return Some(0.0);
        }
        while let Some(sound) = self.events.get(self.next_event) {
            if (sound.time * self.rate as f64).round() as i64 > self.frame {
                break;
            }
            let sound = sound.clone();
            self.next_event += 1;
            self.voice(sound);
        }
        let mut out = [0.0f32; 2];
        for voice in &mut self.voices {
            if voice.position + 1 < voice.pcm.len() {
                out[0] += voice.pcm[voice.position] * voice.gain[0];
                out[1] += voice.pcm[voice.position + 1] * voice.gain[1];
                voice.position += 2;
            }
        }
        self.voices.retain(|v| v.position + 1 < v.pcm.len());
        self.frame += 1;
        self.shared.frame.store(self.frame, Ordering::Relaxed);
        let volume = f32::from_bits(self.shared.volume.load(Ordering::Relaxed));
        self.right = Some((out[1] * volume).clamp(-1.0, 1.0));
        Some((out[0] * volume).clamp(-1.0, 1.0))
    }
}
impl Source for Mixer {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        self.rate
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scheduler_uses_audio_frames_and_pause_freezes_clock() {
        let shared = Arc::new(Shared::new());
        let (tx, rx) = mpsc::channel();
        let mut mixer = Mixer::new(10, shared.clone(), rx);
        let bank = HashMap::from([(0, Arc::from([0.5f32, 0.5, 0.25, 0.25]))]);
        tx.send(Command::Start(
            bank,
            vec![SoundEvent {
                time: 0.0,
                sample: 0,
                volume: 1.0,
                pan: 0.0,
            }],
        ))
        .unwrap();
        for _ in 0..40 {
            assert_eq!(mixer.next(), Some(0.0));
        }
        assert_eq!(shared.frame.load(Ordering::Relaxed), 0);
        shared.paused.store(true, Ordering::Relaxed);
        for _ in 0..10 {
            assert_eq!(mixer.next(), Some(0.0));
        }
        assert_eq!(shared.frame.load(Ordering::Relaxed), 0);
        shared.paused.store(false, Ordering::Relaxed);
        assert!((mixer.next().unwrap() - 0.35).abs() < 0.0001);
        assert!((mixer.next().unwrap() - 0.35).abs() < 0.0001);
    }
}

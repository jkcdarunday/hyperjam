use crate::reader::Reader;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct SoundEvent {
    pub time: f64,
    pub sample: u16,
    pub volume: f32,
    pub pan: f32,
}

#[derive(Clone, Debug)]
pub struct Note {
    pub lane: usize,
    pub sound: SoundEvent,
    pub end: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct Chart {
    pub warnings: Vec<String>,
    pub level: u16,
    pub notes: Vec<Note>,
    pub background: Vec<SoundEvent>,
    pub duration: f64,
}

#[derive(Clone, Debug)]
pub struct Song {
    pub title: String,
    pub artist: String,
    pub noter: String,
    pub sample_file: String,
    pub bpm: f64,
    pub charts: [Chart; 3],
}

#[derive(Clone)]
struct RawEvent {
    measure: u32,
    fraction: f64,
    channel: u16,
    value: f32,
    sample: u16,
    volume: f32,
    pan: f32,
    kind: u8,
}

#[derive(Clone, Debug)]
pub struct SongHeader {
    pub title: String,
    pub artist: String,
    pub noter: String,
    pub sample_file: String,
    pub bpm: f64,
    pub levels: [u16; 3],
}

/// Library indexing only needs the fixed 300-byte header, never the sample bank.
pub fn parse_header(data: &[u8]) -> Result<SongHeader, String> {
    if data.len() < 300 {
        return Err("OJN header is shorter than 300 bytes".into());
    }
    let mut r = Reader::new(data);
    r.bytes(4)?;
    if r.bytes(4)? != b"ojn\0" {
        return Err("Invalid OJN signature".into());
    }
    r.bytes(8)?;
    let bpm = r.f32()? as f64;
    if !bpm.is_finite() || !(1.0..=2000.0).contains(&bpm) {
        return Err("Invalid initial BPM".into());
    }
    let levels = [r.u16()?, r.u16()?, r.u16()?];
    r.pos = 108;
    Ok(SongHeader {
        title: r.text(64)?,
        artist: r.text(32)?,
        noter: r.text(32)?,
        sample_file: r.text(32)?,
        bpm,
        levels,
    })
}

pub fn parse_ojn(data: &[u8]) -> Result<Song, String> {
    let SongHeader {
        title,
        artist,
        noter,
        sample_file,
        bpm,
        levels,
    } = parse_header(data)?;
    let mut r = Reader::new(data);
    r.pos = 64;
    let packages = [r.u32()?, r.u32()?, r.u32()?];
    r.pos = 284;
    let offsets = [
        r.u32()? as usize,
        r.u32()? as usize,
        r.u32()? as usize,
        r.u32()? as usize,
    ];
    let mut charts = Vec::new();
    for d in 0..3 {
        if offsets[d] < 300 || offsets[d] > offsets[d + 1] || offsets[d + 1] > data.len() {
            return Err(format!("Invalid chart offsets for difficulty {}", d + 1));
        }
        let events = read_events(&data[offsets[d]..offsets[d + 1]], packages[d])?;
        charts.push(build_chart(events, bpm, levels[d])?);
    }
    Ok(Song {
        title,
        artist,
        noter,
        sample_file,
        bpm,
        charts: charts.try_into().unwrap(),
    })
}

fn read_events(data: &[u8], packages: u32) -> Result<Vec<RawEvent>, String> {
    if packages as usize > data.len() / 8 {
        return Err("OJN package count exceeds chart size".into());
    }
    let mut r = Reader::new(data);
    let mut events = Vec::new();
    for _ in 0..packages {
        let measure = r.u32()?;
        let channel = r.u16()?;
        let count = r.u16()?;
        if measure > 100_000 {
            return Err("OJN measure index is unreasonably large".into());
        }
        for i in 0..count {
            let bytes = r.bytes(4)?;
            let mut event = RawEvent {
                measure,
                fraction: i as f64 / count as f64,
                channel,
                value: 0.0,
                sample: 0,
                volume: 1.0,
                pan: 0.0,
                kind: 0,
            };
            if channel < 2 {
                event.value = f32::from_le_bytes(bytes.try_into().unwrap());
                if event.value == 0.0 {
                    continue;
                }
                let max = if channel == 0 { 64.0 } else { 2000.0 };
                if !event.value.is_finite() || event.value <= 0.0 || event.value > max {
                    return Err("Invalid OJN timing event".into());
                }
            } else {
                let sample = u16::from_le_bytes(bytes[..2].try_into().unwrap());
                if sample == 0 {
                    continue;
                }
                event.sample = (sample - 1)
                    .checked_add(if bytes[3] % 8 >= 4 { 1000 } else { 0 })
                    .ok_or("Sample ID overflow")?;
                let volume = bytes[2] >> 4;
                event.volume = if volume == 0 {
                    1.0
                } else {
                    volume as f32 / 16.0
                };
                let pan = bytes[2] & 15;
                event.pan = (if pan == 0 { 8 } else { pan } as f32 - 8.0) / 8.0;
                event.kind = bytes[3] % 4;
            }
            events.push(event);
        }
    }
    Ok(events)
}

fn build_chart(mut events: Vec<RawEvent>, mut bpm: f64, level: u16) -> Result<Chart, String> {
    // Channel 0 gives a measure-length multiplier; absent measures are four beats.
    let mut lengths = BTreeMap::new();
    for event in &events {
        if event.channel == 0 {
            lengths.insert(event.measure, event.value as f64 * 4.0);
        }
    }
    let max_measure = events.iter().map(|e| e.measure).max().unwrap_or(0);
    let mut starts = vec![0.0; max_measure as usize + 2];
    for m in 0..=max_measure {
        starts[m as usize + 1] = starts[m as usize] + lengths.get(&m).copied().unwrap_or(4.0);
    }
    let beat = |e: &RawEvent| {
        starts[e.measure as usize] + e.fraction * lengths.get(&e.measure).copied().unwrap_or(4.0)
    };
    events.sort_by(|a, b| beat(a).total_cmp(&beat(b)).then(a.channel.cmp(&b.channel)));
    let mut notes: Vec<Note> = Vec::new();
    let mut background = Vec::new();
    let mut warnings = Vec::new();
    let mut holds: [Option<usize>; 7] = [None; 7];
    let mut time = 0.0;
    let mut last_beat = 0.0;
    for event in &events {
        let this_beat = beat(event);
        time += (this_beat - last_beat) * 60.0 / bpm;
        last_beat = this_beat;
        if event.channel == 0 {
            continue;
        }
        if event.channel == 1 {
            bpm = event.value as f64;
            continue;
        }
        let sound = SoundEvent {
            time,
            sample: event.sample,
            volume: event.volume,
            pan: event.pan,
        };
        if event.channel > 8 {
            if event.kind != 3 {
                background.push(sound);
            }
            continue;
        }
        let lane = (event.channel - 2) as usize;
        match event.kind {
            3 => {
                if let Some(index) = holds[lane].take() {
                    if time > notes[index].sound.time {
                        notes[index].end = Some(time);
                    } else {
                        warnings.push(format!(
                            "Lane {}: zero-length hold treated as a tap",
                            lane + 1
                        ));
                    }
                } else {
                    warnings.push(format!(
                        "Lane {}: orphan hold release treated as a tap",
                        lane + 1
                    ));
                    notes.push(Note {
                        lane,
                        sound,
                        end: None,
                    });
                }
            }
            _ => {
                if let Some(index) = holds[lane].take() {
                    if time > notes[index].sound.time {
                        notes[index].end = Some(time);
                    }
                    warnings.push(format!(
                        "Lane {}: overlapping hold ends at the next note",
                        lane + 1
                    ));
                }
                notes.push(Note {
                    lane,
                    sound,
                    end: None,
                });
                if event.kind == 2 {
                    holds[lane] = Some(notes.len() - 1);
                }
            }
        }
    }
    if holds.iter().any(Option::is_some) {
        warnings.push("Unterminated hold heads treated as taps".into());
    }
    let duration = notes
        .iter()
        .map(|n| n.end.unwrap_or(n.sound.time))
        .chain(background.iter().map(|s| s.time))
        .fold(0.0, f64::max)
        + 3.0;
    Ok(Chart {
        warnings,
        level,
        notes,
        background,
        duration,
    })
}

pub fn practice_song() -> Song {
    let bpm = 132.0;
    let beat = 60.0 / bpm;
    let charts = std::array::from_fn(|difficulty| {
        let mut notes = Vec::new();
        let mut background = Vec::new();
        let step = if difficulty == 0 { 1.0 } else { 0.5 };
        for i in 0..(96.0 / step) as usize {
            let lane = [0, 2, 4, 6, 3, 1, 5, 3][i % 8];
            let time = (4.0 + i as f64 * step) * beat;
            let hold = i % 24 == 20;
            notes.push(Note {
                lane,
                sound: SoundEvent {
                    time,
                    sample: lane as u16,
                    volume: 0.65,
                    pan: (lane as f32 - 3.0) * 0.12,
                },
                end: hold.then_some(time + beat * step * 2.0),
            });
            if difficulty == 2 && i % 4 == 0 {
                notes.push(Note {
                    lane: (lane + 3) % 7,
                    sound: SoundEvent {
                        time,
                        sample: ((lane + 3) % 7) as u16,
                        volume: 0.5,
                        pan: 0.0,
                    },
                    end: None,
                });
            }
        }
        notes.sort_by(|a, b| a.sound.time.total_cmp(&b.sound.time));
        for i in 0..100 {
            background.push(SoundEvent {
                time: i as f64 * beat,
                sample: if i % 4 == 0 { 7 } else { 8 },
                volume: 0.5,
                pan: 0.0,
            });
        }
        Chart {
            warnings: Vec::new(),
            level: [3, 6, 9][difficulty],
            notes,
            background,
            duration: 100.0 * beat + 3.0,
        }
    });
    Song {
        title: "First light".into(),
        artist: "Hyperjam / practice session".into(),
        noter: "Hyperjam".into(),
        sample_file: String::new(),
        bpm,
        charts,
    }
}

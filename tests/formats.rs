use hyperjam::{audio::load_song, chart::parse_ojn, ojm::parse_ojm};

fn put_u16(data: &mut [u8], offset: usize, value: u16) {
    data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn put_u32(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn put_f32(data: &mut [u8], offset: usize, value: f32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn package(measure: u32, channel: u16, events: &[[u8; 4]]) -> Vec<u8> {
    let mut data = vec![];
    data.extend(measure.to_le_bytes());
    data.extend(channel.to_le_bytes());
    data.extend((events.len() as u16).to_le_bytes());
    for event in events {
        data.extend(event);
    }
    data
}
fn sample(value: u16, kind: u8) -> [u8; 4] {
    let [a, b] = value.to_le_bytes();
    [a, b, 0x08, kind]
}

fn ojn_fixture() -> Vec<u8> {
    let mut block = vec![];
    // First measure is two beats long. Initial BPM is 120; at its midpoint it becomes 60.
    block.extend(package(0, 0, &[0.5f32.to_le_bytes()]));
    block.extend(package(0, 1, &[[0; 4], 60.0f32.to_le_bytes()]));
    block.extend(package(0, 2, &[sample(1, 2), [0; 4]]));
    block.extend(package(1, 2, &[sample(1, 3)]));
    block.extend(package(1, 3, &[sample(1, 4)])); // OGG bank reference 1000.
    block.extend(package(1, 9, &[sample(1, 0)])); // Backing sample.
    let mut data = vec![0; 300];
    data[4..8].copy_from_slice(b"ojn\0");
    put_f32(&mut data, 8, 2.9);
    put_f32(&mut data, 16, 120.0);
    // BPM is at byte 16. The header's genre occupies bytes 12..16.
    for d in 0..3 {
        put_u16(&mut data, 20 + d * 2, 3 + d as u16);
        put_u32(&mut data, 64 + d * 4, 6);
        put_u32(&mut data, 284 + d * 4, (300 + block.len() * d) as u32);
    }
    data[108..115].copy_from_slice(b"Fixture");
    data[172..179].copy_from_slice(b"Testers");
    data[236..246].copy_from_slice(b"sounds.ojm");
    put_u32(&mut data, 296, (300 + block.len() * 3) as u32);
    for _ in 0..3 {
        data.extend(&block);
    }
    data
}

fn ojm_fixture() -> Vec<u8> {
    let mut data = vec![0; 20 + 56 + 8];
    data[..4].copy_from_slice(b"OJM\0");
    put_u32(&mut data, 8, 20);
    let len = data.len() as u32;
    put_u32(&mut data, 12, len);
    put_u32(&mut data, 16, len);
    put_u16(&mut data, 52, 1);
    put_u16(&mut data, 54, 1);
    put_u32(&mut data, 56, 44100);
    put_u32(&mut data, 60, 88200);
    put_u16(&mut data, 64, 2);
    put_u16(&mut data, 66, 16);
    put_u32(&mut data, 72, 8);
    for (i, value) in [0i16, 16000, -16000, 0].iter().enumerate() {
        data[76 + i * 2..78 + i * 2].copy_from_slice(&value.to_le_bytes());
    }
    data
}

#[test]
fn ojn_tempo_measure_lengths_holds_and_sample_banks() {
    let song = parse_ojn(&ojn_fixture()).unwrap();
    assert_eq!(song.title, "Fixture");
    assert_eq!(song.sample_file, "sounds.ojm");
    for chart in &song.charts {
        assert_eq!(chart.notes.len(), 2);
        assert_eq!(chart.background.len(), 1);
        assert_eq!(chart.notes[0].sound.time, 0.0);
        assert_eq!(chart.notes[0].end, Some(1.5));
        assert_eq!(chart.notes[1].sound.time, 1.5);
        assert_eq!(chart.notes[1].sound.sample, 1000);
    }
}

#[test]
fn ojm_pcm_is_reconstructed_as_a_decodable_wav() {
    let samples = parse_ojm(&ojm_fixture()).unwrap();
    let wav = &samples[&0];
    assert_eq!(&wav[..4], b"RIFF");
    assert_eq!(&wav[8..12], b"WAVE");
    assert_eq!(wav.len(), 52);
}

#[test]
fn m30_decrypts_both_masks_and_preserves_partial_word() {
    for (flag, mask) in [(16, b"nami"), (32, b"0412")] {
        let mut data = vec![0; 28 + 52 + 7];
        data[..4].copy_from_slice(b"M30\0");
        put_u32(&mut data, 8, flag);
        put_u32(&mut data, 12, 1);
        put_u32(&mut data, 16, 28);
        put_u32(&mut data, 60, 7);
        put_u16(&mut data, 64, 0);
        put_u16(&mut data, 72, 5);
        data[80..87].copy_from_slice(b"OggSabc");
        for i in 0..4 {
            data[80 + i] ^= mask[i];
        }
        let samples = parse_ojm(&data).unwrap();
        assert_eq!(&samples[&1005], b"OggSabc");
    }
}

#[test]
fn omc_decrypts_permuted_pcm_blocks() {
    let encoded = [
        64, 65, 189, 188, 199, 198, 58, 59, 247, 246, 245, 244, 219, 218, 217, 216, 16, 17, 18, 19,
        255, 254, 253, 252, 227, 29, 30, 31, 251, 250, 249, 248, 231, 230, 229, 27, 32, 33, 34,
        220, 195, 61, 62, 63, 215, 214, 42, 212, 235, 234, 233, 232, 48, 49, 205, 51, 243, 13, 14,
        15, 203, 202, 201, 200, 211, 45, 46, 47,
    ];
    let mut data = ojm_fixture();
    data[..4].copy_from_slice(b"OMC\0");
    data.truncate(76);
    data.extend(encoded);
    let len = data.len() as u32;
    put_u32(&mut data, 12, len);
    put_u32(&mut data, 16, len);
    put_u32(&mut data, 72, 68);
    let bank = parse_ojm(&data).unwrap();
    assert_eq!(&bank[&0][44..], &(0u8..68).collect::<Vec<_>>());
}

#[test]
fn truncated_and_malformed_inputs_return_errors_without_panicking() {
    let ojn = ojn_fixture();
    for end in 0..ojn.len() {
        assert!(parse_ojn(&ojn[..end]).is_err());
    }
    let ojm = ojm_fixture();
    for end in 0..ojm.len() {
        assert!(parse_ojm(&ojm[..end]).is_err());
    }
    let mut corrupt = ojn.clone();
    put_f32(&mut corrupt, 16, f32::NAN);
    assert!(parse_ojn(&corrupt).is_err());
    let mut corrupt = ojn.clone();
    put_u32(&mut corrupt, 284, u32::MAX);
    assert!(parse_ojn(&corrupt).is_err());
    let mut corrupt = ojn.clone();
    put_u32(&mut corrupt, 64, u32::MAX);
    assert!(parse_ojn(&corrupt).is_err());
    let mut corrupt = ojn.clone();
    put_u32(&mut corrupt, 300, u32::MAX);
    assert!(parse_ojn(&corrupt).is_err());
}

#[test]
fn disk_loading_finds_case_insensitive_sample_filename_and_decodes_pcm() {
    let dir = std::env::temp_dir().join(format!("hyperjam-formats-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("chart.ojn"), ojn_fixture()).unwrap();
    std::fs::write(dir.join("SOUNDS.OJM"), ojm_fixture()).unwrap();
    let loaded = load_song(&dir.join("chart.ojn"), None, 48000).unwrap();
    assert_eq!(loaded.bank.len(), 1);
    assert!(!loaded.bank[&0].is_empty());
    assert!(
        loaded
            .warnings
            .iter()
            .any(|w| w.contains("1 referenced samples"))
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn practice_autoplay_achieves_a_full_combo_on_all_difficulties() {
    for chart in hyperjam::chart::practice_song().charts {
        let expected = chart.notes.len() + chart.notes.iter().filter(|n| n.end.is_some()).count();
        let mut game = hyperjam::gameplay::Gameplay::new(chart, true);
        for tick in 0..5000 {
            game.update(tick as f64 * 0.01);
        }
        assert_eq!(game.counts[3], 0);
        assert_eq!(game.max_combo, expected);
        assert_eq!(game.accuracy(), 100.0);
    }
}

#[test]
fn library_indexes_subfolders_and_searches_metadata_and_filenames() {
    let dir = std::env::temp_dir().join(format!("hyperjam-library-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("one.ojn"), ojn_fixture()).unwrap();
    std::fs::write(dir.join("sub/two.OJN"), ojn_fixture()).unwrap();
    std::fs::write(dir.join("bad.ojn"), b"invalid").unwrap();
    std::fs::write(dir.join("bank.ojm"), ojm_fixture()).unwrap();
    let library = hyperjam::library::scan(&dir).unwrap();
    assert_eq!(library.entries.len(), 2);
    assert_eq!(library.warnings.len(), 1);
    assert_eq!(library.filter("TESTERS").len(), 2);
    assert_eq!(library.filter("two.ojn").len(), 1);
    assert!(library.filter("missing").is_empty());
    assert_eq!(library.entries[0].header.levels, [3, 4, 5]);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn orphan_releases_and_overlapping_heads_are_playable_with_warnings() {
    let mut orphan = ojn_fixture();
    // Package sizes before the first note: channel 0 = 12, channel 1 = 16.
    orphan[300 + 12 + 16 + 8 + 3] = 0;
    let song = parse_ojn(&orphan).unwrap();
    assert_eq!(song.charts[0].notes.len(), 3);
    assert!(song.charts[0].warnings[0].contains("orphan"));
    let mut overlap = ojn_fixture();
    overlap[300 + 12 + 16 + 8 + 4..300 + 12 + 16 + 8 + 8].copy_from_slice(&sample(1, 2));
    let song = parse_ojn(&overlap).unwrap();
    assert_eq!(song.charts[0].notes[0].end, Some(0.5));
    assert_eq!(song.charts[0].notes[1].end, Some(1.5));
    assert!(song.charts[0].warnings[0].contains("overlapping"));
}

#[test]
fn m30_overstated_record_count_accepts_only_complete_records() {
    let mut data = vec![0; 28 + 52 + 4];
    data[..4].copy_from_slice(b"M30\0");
    put_u32(&mut data, 12, 1000);
    put_u32(&mut data, 16, 28);
    put_u32(&mut data, 60, 4);
    put_u16(&mut data, 64, 5);
    data[80..84].copy_from_slice(b"OggS");
    // Slot counts may exceed even the minimum record bytes in a sparse bank.
    assert_eq!(parse_ojm(&data).unwrap()[&0], b"OggS");
    assert!(parse_ojm(&data[..83]).is_err());
}

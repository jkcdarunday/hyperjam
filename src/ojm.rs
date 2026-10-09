//! OJM, OMC and M30 sample containers. IDs match OJN's zero-based sample references.
use crate::reader::Reader;
use std::collections::HashMap;

pub type EncodedBank = HashMap<u16, Vec<u8>>;
const MAX_SAMPLE: usize = 64 * 1024 * 1024;

pub fn parse_ojm(data: &[u8]) -> Result<EncodedBank, String> {
    let signature = data.get(..4).ok_or("Truncated OJM signature")?;
    match signature {
        b"M30\0" => m30(data),
        b"OJM\0" => omc(data, false),
        b"OMC\0" => omc(data, true),
        _ => Err("Unknown sample container (expected OJM, OMC or M30)".into()),
    }
}

fn payload(r: &mut Reader, size: usize) -> Result<Vec<u8>, String> {
    if size > MAX_SAMPLE {
        return Err("Sample exceeds the 64 MiB safety limit".into());
    }
    Ok(r.bytes(size)?.to_vec())
}

fn m30(data: &[u8]) -> Result<EncodedBank, String> {
    let mut r = Reader::new(data);
    r.bytes(8)?;
    let encryption = r.u32()?;
    let count = r.u32()?;
    let offset = r.u32()? as usize;
    r.bytes(8)?;
    if ![0, 16, 32].contains(&encryption) {
        return Err(format!("Unsupported M30 encryption flag: {encryption}"));
    }
    r.pos = if offset == 0 { 28 } else { offset };
    if r.pos < 28 || r.pos > data.len() {
        return Err("Invalid M30 sample offset/count".into());
    }
    let mut bank = HashMap::new();
    for _ in 0..count {
        // Some official banks report the available reference slots rather than
        // the number of stored records. Only accept EOF at a complete boundary.
        if r.pos == data.len() {
            break;
        }
        r.bytes(32)?;
        let size = r.u32()? as usize;
        let codec = r.u16()?;
        r.bytes(6)?;
        let reference = r.u16()?;
        r.bytes(6)?;
        let mut bytes = payload(&mut r, size)?;
        let mask = match encryption {
            16 => Some(b"nami"),
            32 => Some(b"0412"),
            _ => None,
        };
        if let Some(mask) = mask {
            for chunk in bytes.chunks_exact_mut(4) {
                for (b, m) in chunk.iter_mut().zip(mask) {
                    *b ^= m;
                }
            }
        }
        let id = match codec {
            0 => reference.checked_add(1000).ok_or("Sample ID overflow")?,
            5 => reference,
            _ => return Err(format!("Unsupported M30 codec: {codec}")),
        };
        if size > 0 {
            bank.insert(id, bytes);
        }
    }
    Ok(bank)
}

// The OMC block permutation is format data, documented by the Open2Jam project.
const PERMUTATION: [u8; 289] = [
    16, 14, 2, 9, 4, 0, 7, 1, 6, 8, 15, 10, 5, 12, 3, 13, 11, 7, 2, 10, 11, 3, 5, 13, 8, 4, 0, 12,
    6, 15, 14, 16, 1, 9, 12, 13, 3, 0, 6, 9, 10, 1, 7, 8, 16, 2, 11, 14, 4, 15, 5, 8, 3, 4, 13, 6,
    5, 11, 16, 2, 12, 7, 9, 10, 15, 14, 0, 1, 15, 2, 12, 13, 0, 4, 1, 5, 7, 3, 9, 16, 6, 11, 10, 8,
    14, 0, 4, 11, 16, 15, 13, 12, 6, 5, 7, 1, 2, 3, 8, 9, 10, 14, 3, 16, 8, 7, 6, 9, 14, 13, 0, 10,
    11, 4, 5, 12, 2, 1, 15, 4, 14, 16, 15, 5, 8, 7, 11, 0, 1, 6, 2, 12, 9, 3, 10, 13, 6, 13, 14, 7,
    16, 10, 11, 0, 1, 12, 15, 2, 3, 8, 9, 4, 5, 10, 12, 0, 8, 9, 13, 3, 4, 5, 16, 14, 15, 1, 2, 11,
    6, 7, 5, 6, 12, 4, 13, 15, 7, 14, 8, 1, 9, 2, 16, 10, 11, 0, 3, 11, 15, 4, 14, 3, 1, 0, 2, 13,
    12, 6, 7, 5, 16, 9, 8, 10, 3, 2, 1, 0, 4, 12, 13, 11, 16, 5, 6, 15, 14, 7, 9, 10, 8, 9, 10, 0,
    7, 8, 6, 16, 3, 4, 1, 2, 5, 11, 14, 15, 13, 12, 10, 6, 9, 12, 11, 16, 7, 8, 0, 15, 3, 1, 2, 5,
    13, 14, 4, 13, 0, 1, 14, 2, 3, 8, 11, 7, 12, 9, 5, 10, 15, 4, 6, 16, 1, 14, 2, 3, 13, 11, 7, 0,
    8, 12, 9, 6, 15, 16, 5, 10, 4,
];

#[derive(Default)]
struct Cipher {
    key: u8,
    bit: u8,
}
impl Cipher {
    fn decrypt(&mut self, encoded: &[u8]) -> Vec<u8> {
        let size = encoded.len() / 17;
        let row = (encoded.len() % 17) * 17;
        let mut plain = encoded.to_vec();
        for block in 0..17 {
            let destination = PERMUTATION[row + block] as usize * size;
            plain[destination..destination + size]
                .copy_from_slice(&encoded[block * size..(block + 1) * size]);
        }
        for byte in &mut plain {
            let original = *byte;
            if (self.key << self.bit) & 0x80 != 0 {
                *byte = !*byte;
            }
            self.bit += 1;
            if self.bit == 8 {
                self.bit = 0;
                self.key = original;
            }
        }
        plain
    }
}

fn omc(data: &[u8], encrypted: bool) -> Result<EncodedBank, String> {
    let mut r = Reader::new(data);
    r.bytes(8)?;
    let wav_start = r.u32()? as usize;
    let ogg_start = r.u32()? as usize;
    let file_size = r.u32()? as usize;
    let start = if wav_start == 0 { 20 } else { wav_start };
    if start < 20 || start > ogg_start || ogg_start > file_size || file_size > data.len() {
        return Err("Invalid OJM section offsets".into());
    }
    let mut cipher = Cipher { key: 0xff, bit: 0 };
    let mut bank = HashMap::new();
    r.pos = start;
    let mut id = 0u16;
    while r.pos < ogg_start {
        if ogg_start - r.pos < 56 {
            return Err("Truncated OJM WAV header".into());
        }
        r.bytes(32)?;
        let format = r.bytes(16)?.to_vec();
        r.bytes(4)?;
        let size = r.u32()? as usize;
        if size > ogg_start - r.pos {
            return Err("WAV sample extends into OGG section".into());
        }
        let raw = payload(&mut r, size)?;
        if !raw.is_empty() {
            let pcm = if encrypted { cipher.decrypt(&raw) } else { raw };
            let mut wav = Vec::with_capacity(44 + pcm.len());
            wav.extend(b"RIFF");
            wav.extend((36 + pcm.len() as u32).to_le_bytes());
            wav.extend(b"WAVEfmt ");
            wav.extend(16u32.to_le_bytes());
            wav.extend(format);
            wav.extend(b"data");
            wav.extend((pcm.len() as u32).to_le_bytes());
            wav.extend(pcm);
            bank.insert(id, wav);
        }
        id = id.checked_add(1).ok_or("Too many WAV samples")?;
    }
    r.pos = ogg_start;
    id = 1000;
    while r.pos < file_size {
        if file_size - r.pos < 36 {
            return Err("Truncated OJM OGG header".into());
        }
        r.bytes(32)?;
        let size = r.u32()? as usize;
        if size > file_size - r.pos {
            return Err("OGG sample extends past container".into());
        }
        let bytes = payload(&mut r, size)?;
        if !bytes.is_empty() {
            bank.insert(id, bytes);
        }
        id = id.checked_add(1).ok_or("Too many OGG samples")?;
    }
    Ok(bank)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_permutation_row_is_complete() {
        for row in PERMUTATION.chunks(17) {
            let mut row = row.to_vec();
            row.sort();
            assert_eq!(row, (0..17).collect::<Vec<u8>>());
        }
    }
    #[test]
    fn cipher_state_spans_sample_boundaries() {
        let mut cipher = Cipher { key: 255, bit: 0 };
        assert_eq!(cipher.decrypt(&[1, 2, 3]), vec![254, 253, 252]);
        assert_eq!(
            cipher.decrypt(&[4, 5, 6, 7, 8, 9]),
            vec![251, 250, 249, 248, 247, 9]
        );
    }
}

use crate::{Error, HotCue, Loop, Result};
use base64::Engine;

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MarkerEntry {
    pub kind: String,
    pub data: Vec<u8>,
}
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Markers {
    pub cues: Vec<HotCue>,
    pub loops: Vec<Loop>,
    pub color: Option<[u8; 3]>,
    pub beat_grid_locked: Option<bool>,
    pub unknown: Vec<MarkerEntry>,
}
impl Markers {
    /// Legacy positions and colors override slots 1–5/1–9; Markers2 supplies labels.
    pub fn merge_legacy(&mut self, mut old: Self) {
        for cue in &mut old.cues {
            if let Some(new) = self.cues.iter().find(|new| new.slot == cue.slot) {
                cue.name = new.name.clone();
            }
        }
        for value in &mut old.loops {
            if let Some(new) = self.loops.iter().find(|new| new.slot == value.slot) {
                value.name = new.name.clone();
            }
        }
        self.cues.retain(|cue| cue.slot > 5);
        self.loops.retain(|value| value.slot > 9);
        self.cues.extend(old.cues);
        self.loops.extend(old.loops);
        self.cues.sort_by_key(|cue| cue.slot);
        self.loops.sort_by_key(|value| value.slot);
        self.color = old.color.or(self.color);
    }
}
fn invalid(message: &str) -> Error {
    Error::Invalid(format!("invalid Serato markers: {message}"))
}
fn number(data: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_be_bytes(
        data.get(offset..offset + 4)
            .ok_or_else(|| invalid("truncated number"))?
            .try_into()
            .unwrap(),
    ))
}
fn name(data: &[u8], offset: usize) -> Result<String> {
    let text = data
        .get(offset..)
        .ok_or_else(|| invalid("truncated name"))?;
    let length = text
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| invalid("unterminated name"))?;
    if text[length + 1..].iter().any(|byte| *byte != 0) {
        return Err(invalid("data after name"));
    }
    Ok(std::str::from_utf8(&text[..length])
        .map_err(|_| invalid("name is not UTF-8"))?
        .into())
}
fn slot(index: u8) -> Result<u8> {
    index
        .checked_add(1)
        .ok_or_else(|| invalid("slot exceeds portable limit"))
}
pub fn parse_markers2(data: &[u8]) -> Result<Markers> {
    if !data.starts_with(&[1, 1]) {
        return Err(invalid("Markers2 header"));
    }
    let mut encoded: Vec<_> = data[2..]
        .iter()
        .copied()
        .take_while(|byte| *byte != 0)
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    if encoded.len() % 4 == 1 {
        encoded.push(b'A');
    }
    while !encoded.len().is_multiple_of(4) {
        encoded.push(b'=');
    }
    let raw = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| invalid("base64 payload"))?;
    if !raw.starts_with(&[1, 1]) {
        return Err(invalid("decoded Markers2 header"));
    }
    let mut bytes = &raw[2..];
    let mut markers = Markers::default();
    loop {
        let end = bytes
            .iter()
            .position(|byte| *byte == 0)
            .ok_or_else(|| invalid("unterminated entry type"))?;
        if end == 0 {
            if bytes.iter().any(|byte| *byte != 0) {
                return Err(invalid("data after terminator"));
            }
            break;
        }
        let kind =
            std::str::from_utf8(&bytes[..end]).map_err(|_| invalid("entry type is not UTF-8"))?;
        let length = number(bytes, end + 1)? as usize;
        let start = end + 5;
        let entry = bytes
            .get(start..)
            .and_then(|remaining| remaining.get(..length))
            .ok_or_else(|| invalid("truncated entry"))?;
        bytes = &bytes[start + length..];
        match kind {
            "CUE" => {
                if entry.len() < 13 {
                    return Err(invalid("short cue"));
                }
                markers.cues.push(HotCue {
                    name: name(entry, 12)?,
                    offset: f64::from(number(entry, 2)?) / 1000.0,
                    slot: slot(entry[1])?,
                    color: entry[7..10].try_into().unwrap(),
                });
            }
            "LOOP" => {
                if entry.len() < 21 {
                    return Err(invalid("short loop"));
                }
                let start = f64::from(number(entry, 2)?) / 1000.0;
                let end = f64::from(number(entry, 6)?) / 1000.0;
                if end < start {
                    return Err(invalid("loop ends before start"));
                }
                markers.loops.push(Loop {
                    name: name(entry, 20)?,
                    start,
                    end,
                    slot: slot(entry[1])?,
                    color: entry[15..18].try_into().unwrap(),
                    locked: entry[19] != 0,
                });
            }
            "COLOR" if entry.len() == 4 => markers.color = Some(entry[1..4].try_into().unwrap()),
            "BPMLOCK" if entry.len() == 1 => markers.beat_grid_locked = Some(entry[0] != 0),
            "COLOR" | "BPMLOCK" => return Err(invalid("invalid scalar entry length")),
            _ => markers.unknown.push(MarkerEntry {
                kind: kind.into(),
                data: entry.to_vec(),
            }),
        }
    }
    validate_slots(&markers)?;
    Ok(markers)
}
fn unpack(data: &[u8]) -> [u8; 3] {
    let [w, x, y, z]: [u8; 4] = data.try_into().unwrap();
    [
        ((x & 127) >> 2) | ((w & 7) << 5),
        ((y & 127) >> 1) | ((x & 3) << 6),
        (z & 127) | ((y & 1) << 7),
    ]
}
fn position(data: &[u8]) -> f64 {
    let [a, b, c] = unpack(data);
    f64::from(u32::from_be_bytes([0, a, b, c])) / 1000.0
}
pub fn parse_markers_(data: &[u8]) -> Result<Markers> {
    if data.len() != 318 || !data.starts_with(&[2, 5]) || number(data, 2)? != 14 {
        return Err(invalid("legacy header or length"));
    }
    let mut markers = Markers {
        color: Some(unpack(&data[314..318])),
        ..Markers::default()
    };
    for (index, entry) in data[6..314].as_chunks::<22>().0.iter().enumerate() {
        if entry[20] != if index < 5 { 1 } else { 3 }
            || !matches!(entry[0], 0 | 127)
            || !matches!(entry[5], 0 | 127)
        {
            return Err(invalid("legacy entry type or flags"));
        }
        if entry[0] == 127 {
            continue;
        }
        let start = position(&entry[1..5]);
        let color = unpack(&entry[16..20]);
        if index < 5 {
            markers.cues.push(HotCue {
                slot: index as u8 + 1,
                offset: start,
                color,
                ..HotCue::default()
            });
        } else {
            if entry[5] == 127 {
                return Err(invalid("loop has no end"));
            }
            let end = position(&entry[6..10]);
            if end < start {
                return Err(invalid("loop ends before start"));
            }
            markers.loops.push(Loop {
                slot: index as u8 - 4,
                start,
                end,
                color,
                locked: entry[21] != 0,
                ..Loop::default()
            });
        }
    }
    Ok(markers)
}
fn validate_slots(markers: &Markers) -> Result<()> {
    let mut cues = std::collections::BTreeSet::new();
    let mut loops = std::collections::BTreeSet::new();
    if markers.cues.iter().any(|cue| !cues.insert(cue.slot))
        || markers.loops.iter().any(|value| !loops.insert(value.slot))
    {
        return Err(invalid("duplicate marker slot"));
    }
    Ok(())
}

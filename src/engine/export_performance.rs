use super::export::ExportOptions;
use crate::{Error, Result, Song};
use engine_library::performance::{BeatData, BeatMarker, Cue, Loop, PerformanceData, q_compress};
pub(super) fn performance(
    song: &Song,
    id: i64,
    options: &ExportOptions,
) -> Result<Option<PerformanceData>> {
    let original = song
        .engine
        .as_ref()
        .and_then(|native| native.performance.clone());
    if original.is_none()
        && song.grid.is_empty()
        && song.cues.is_empty()
        && song.loops.is_empty()
        && song.cue.is_none()
        && song.track_data.is_none()
        && song.overview_wave_form_data.is_none()
    {
        return Ok(None);
    }
    let mut raw = original.clone().unwrap_or_default();
    let decoded = raw.decode(options.decompression_limit)?;
    let rate = song.sample_rate.unwrap_or(options.fallback_sample_rate);
    if !rate.is_finite() || rate <= 0.0 {
        return Err(Error::Invalid("invalid sample rate".into()));
    }
    if original.is_some()
        && decoded
            .beats
            .as_ref()
            .is_some_and(|beats| beats.sample_rate != rate)
    {
        return Err(Error::Invalid(
            "changing a native sample rate requires reanalysis of original positions and opaque caches".into(),
        ));
    }
    let same_rate = decoded
        .beats
        .as_ref()
        .is_some_and(|beats| beats.sample_rate == rate);
    if !same_rate
        || decoded
            .beats
            .as_ref()
            .map(|beats| super::import::grid(rate, &beats.adjusted))
            .transpose()?
            .unwrap_or_default()
            != song.grid
    {
        let markers = markers(song, rate)?;
        let mut beats = decoded.beats.unwrap_or(BeatData {
            sample_rate: rate,
            track_samples: song.length * rate,
            is_set: u8::from(!markers.is_empty()),
            original: markers.clone(),
            adjusted: Vec::new(),
            trailing: Vec::new(),
        });
        beats.sample_rate = rate;
        beats.adjusted = markers;
        raw.beat_data = Some(q_compress(&beats.encode()?)?);
    }
    let mut cues = decoded.cues.unwrap_or_default();
    let mut slots = std::array::from_fn(|_| None);
    for cue in &song.cues {
        let slot = usize::from(cue.slot - 1);
        if slot >= 8 {
            return Err(Error::Invalid("Engine supports eight hot cue slots".into()));
        }
        slots[slot] = Some(Cue {
            name: cue.name.clone(),
            samples: cue.offset * rate,
            color: [
                cues.slots[slot]
                    .as_ref()
                    .map(|cue| cue.color[0])
                    .unwrap_or(255),
                cue.color[0],
                cue.color[1],
                cue.color[2],
            ],
        });
    }
    let main = song.cue.map(|cue| cue * rate).unwrap_or(-1.0);
    if cues.slots != slots || cues.modified != main || !same_rate {
        if original
            .as_ref()
            .and_then(|raw| raw.quick_cues.as_ref())
            .is_none()
        {
            cues.original = main;
        }
        cues.slots = slots;
        cues.modified = main;
        raw.quick_cues = Some(q_compress(&cues.encode()?)?);
    }
    let mut loops = decoded.loops.unwrap_or_default();
    let mut slots = std::array::from_fn(|_| None);
    for value in &song.loops {
        let slot = usize::from(value.slot - 1);
        if slot >= 8 {
            return Err(Error::Invalid("Engine supports eight loop slots".into()));
        }
        if value.locked {
            return Err(Error::Invalid(
                "Engine loop lock encoding is undocumented".into(),
            ));
        }
        slots[slot] = Some(Loop {
            name: value.name.clone(),
            start: value.start * rate,
            end: value.end * rate,
            color: value.color,
            flags: loops.slots[slot]
                .as_ref()
                .map(|value| value.flags)
                .unwrap_or([0; 3]),
        });
    }
    if loops.slots != slots || !same_rate {
        loops.slots = slots;
        raw.loops = Some(loops.encode()?);
    }
    raw.track_id = id;
    raw.track_data = song.track_data.clone();
    raw.overview_wave_form_data = song.overview_wave_form_data.clone();
    raw.active_on_load_loops = song.active_on_load_loops;
    Ok(Some(raw))
}
fn markers(song: &Song, rate: f64) -> Result<Vec<BeatMarker>> {
    let mut result = Vec::new();
    for (index, marker) in song.grid.iter().enumerate() {
        let beats = if let Some(next) = song.grid.get(index + 1) {
            (next.start - marker.start) * marker.bpm / 60.0
        } else {
            1.0
        };
        if !beats.is_finite()
            || beats <= 0.0
            || beats > u32::MAX as f64
            || (beats - beats.round()).abs() > 1e-6
        {
            return Err(Error::Invalid(
                "Engine grid segments require a positive integral number of beats".into(),
            ));
        }
        result.push(BeatMarker {
            samples: marker.start * rate,
            beat: i64::from(marker.beat),
            beats_to_next: beats.round() as u32,
            unknown: 0,
        });
    }
    if let Some(last) = song.grid.last() {
        result.push(BeatMarker {
            samples: (last.start + 60.0 / last.bpm) * rate,
            beat: i64::from((last.beat + 1) % 4),
            beats_to_next: 0,
            unknown: 0,
        });
    }
    Ok(result)
}

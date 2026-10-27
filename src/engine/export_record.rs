use super::export::ExportOptions;
use crate::{Error, Result, Song};
use engine_library::{model::DurationUnit, records::TrackRecord};
fn merge<T: Clone + PartialEq + Default>(original: &mut Option<T>, value: &T) {
    if original.as_ref().cloned().unwrap_or_default() != *value {
        *original = Some(value.clone());
    }
}
fn integer(value: f64, field: &str) -> Result<i64> {
    if !value.is_finite() || value < 0.0 || value >= i64::MAX as f64 {
        return Err(Error::Invalid(format!("{field} is out of range")));
    }
    Ok(value as i64)
}
pub(super) fn record(song: &Song, id: i64, options: &ExportOptions) -> Result<TrackRecord> {
    let mut record = song
        .engine
        .as_ref()
        .map(|native| native.record.clone())
        .unwrap_or_default();
    let t = &mut record.track;
    let m = &mut record.metadata;
    t.id = id;
    for (field, value) in [
        (&mut t.title, &song.title),
        (&mut t.artist, &song.artist),
        (&mut t.album, &song.album),
        (&mut t.genre, &song.genre),
        (&mut t.comment, &song.comment),
        (&mut t.label, &song.label),
        (&mut t.composer, &song.composer),
        (&mut t.remixer, &song.remixer),
        (&mut t.file_type, &song.file_type),
    ] {
        merge(field, value);
    }
    let path = song
        .path
        .to_str()
        .ok_or_else(|| Error::Invalid("Engine SQLite paths require UTF-8".into()))?
        .to_owned();
    merge(&mut t.path, &path);
    if song.engine.is_none() {
        t.filename = song
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned);
    }
    let length = integer(
        song.length
            * match options.duration_unit {
                DurationUnit::Seconds => 1.0,
                DurationUnit::Milliseconds => 1000.0,
            },
        "duration",
    )?;
    merge(&mut t.length, &length);
    t.bpm = song.bpm.map(|bpm| integer(bpm, "BPM")).transpose()?;
    t.bpm_analyzed = song.bpm_analyzed;
    t.year = song.year.map(i64::from);
    merge(
        &mut t.file_bytes,
        &i64::try_from(song.size)
            .map_err(|_| Error::Invalid("file size exceeds SQLite integer".into()))?,
    );
    merge(&mut t.bitrate, &i64::from(song.bitrate));
    merge(&mut t.rating, &i64::from(song.rating));
    t.key = song.key.map(i64::from);
    t.album_art_id = song.album_art_id;
    t.album_art_source_hash = song.album_art_source_hash.clone();
    merge_date(&mut t.last_edit_time, song.date_modified)?;
    merge(&mut t.is_analyzed, &song.is_analyzed);
    merge_date(&mut m.time_last_played, song.time_last_played)?;
    merge_date(&mut m.date_added, song.date_added)?;
    merge_date(&mut m.date_created, song.date_created)?;
    merge(&mut m.is_played, &song.is_played);
    merge(&mut m.played_indicator, &song.played_indicator);
    merge(&mut m.is_beat_grid_locked, &song.is_beat_grid_locked);
    merge(&mut m.explicit_lyrics, &song.explicit_lyrics);
    m.origin_database_uuid = song.origin_database_uuid.clone();
    m.origin_track_id = song.origin_track_id;
    m.streaming_source = song.streaming_source.clone();
    m.uri = song.uri.clone();
    m.streaming_flags = song.streaming_flags;
    Ok(record)
}

fn merge_date(
    original: &mut Option<String>,
    value: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<()> {
    let parsed = original
        .as_deref()
        .map(engine_library::records::parse_timestamp)
        .transpose()?
        .flatten();
    if parsed != value {
        *original = value.map(|date| date.to_rfc3339());
    }
    Ok(())
}

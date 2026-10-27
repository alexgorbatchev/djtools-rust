use crate::{
    EngineRecord, Error, HotCue, Library, Loop, Marker, Playlist, Result, Song, check_cancel,
};
use engine_library::{
    Database,
    collection::PlaylistNode,
    model::DurationUnit,
    records::{TrackRecord, parse_timestamp},
};
use std::{collections::BTreeMap, path::Path};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug)]
pub struct ImportOptions {
    pub preserve_paths: bool,
    pub original_grids: bool,
    pub original_cues: bool,
    pub duration_unit: DurationUnit,
    pub decompression_limit: usize,
    pub fallback_sample_rate: f64,
}
impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            preserve_paths: false,
            original_grids: false,
            original_cues: false,
            duration_unit: DurationUnit::Seconds,
            decompression_limit: 64 * 1024 * 1024,
            fallback_sample_rate: 44100.0,
        }
    }
}

pub fn import(
    path: impl AsRef<Path>,
    options: &ImportOptions,
    token: &CancellationToken,
) -> Result<Library> {
    check_cancel(token)?;
    let base = path.as_ref();
    let nested = base.join("Database2/m.db");
    let database_path = match nested.try_exists()? {
        true => nested,
        false => base.join("m.db"),
    };
    let database = Database::open(&database_path, true)?;
    let information = database.information()?;
    let mut library = Library {
        database_uuid: information.as_ref().map(|i| i.uuid),
        schema_version: information.map(|i| i.schema_version),
        artwork: database.artwork()?,
        smartlists: database.smartlists()?,
        ..Library::default()
    };
    for record in database.track_records()? {
        check_cancel(token)?;
        let mut song = song_from_record(&record, base, options)?;
        let performance = database.performance(song.id)?;
        if let Some(raw) = &performance {
            if !options.fallback_sample_rate.is_finite() || options.fallback_sample_rate <= 0.0 {
                return Err(Error::Invalid("invalid fallback sample rate".into()));
            }
            song.sample_rate = Some(options.fallback_sample_rate);
            song.track_data = raw.track_data.clone();
            song.overview_wave_form_data = raw.overview_wave_form_data.clone();
            song.active_on_load_loops = raw.active_on_load_loops;
            let decoded = raw.decode(options.decompression_limit)?;
            if let Some(beats) = decoded.beats {
                song.sample_rate = Some(beats.sample_rate);
                song.grid = grid(
                    beats.sample_rate,
                    if options.original_grids {
                        &beats.original
                    } else {
                        &beats.adjusted
                    },
                )?;
            }
            if let Some(cues) = decoded.cues {
                let rate = sample_rate(&song)?;
                let main = if options.original_cues {
                    cues.original
                } else {
                    cues.modified
                };
                song.cue = (main >= 0.0).then_some(main / rate);
                for (slot, cue) in cues.slots.into_iter().enumerate() {
                    if let Some(cue) = cue {
                        song.cues.push(HotCue {
                            name: cue.name,
                            offset: cue.samples / rate,
                            slot: slot as u8 + 1,
                            color: [cue.color[1], cue.color[2], cue.color[3]],
                        });
                    }
                }
            }
            if let Some(loops) = decoded.loops {
                // Empty slots carry no positions and do not require a sample rate.
                if loops.slots.iter().any(Option::is_some) {
                    let rate = sample_rate(&song)?;
                    for (slot, value) in loops.slots.into_iter().enumerate() {
                        if let Some(value) = value {
                            song.loops.push(Loop {
                                name: value.name,
                                start: value.start / rate,
                                end: value.end / rate,
                                slot: slot as u8 + 1,
                                color: value.color,
                                locked: false,
                            });
                        }
                    }
                }
            }
        }
        song.engine = Some(EngineRecord {
            record,
            performance,
        });
        library.songs.push(song);
    }
    library.playlists = engine_library::collection::hierarchy(&database)?
        .iter()
        .map(|node| playlist(node, &database, token))
        .collect::<Result<_>>()?;
    let history_path = database_path.with_file_name("hm.db");
    if history_path.try_exists()? {
        let history_database = Database::open(history_path, true)?;
        let history: BTreeMap<_, _> = engine_library::history::read(history_database.connection())?
            .into_iter()
            .map(|entry| {
                (
                    (entry.origin_database_uuid.clone(), entry.origin_track_id),
                    entry,
                )
            })
            .collect();
        for song in &mut library.songs {
            check_cancel(token)?;
            let identity = (
                song.origin_database_uuid.clone(),
                song.origin_track_id
                    .filter(|id| *id != 0)
                    .unwrap_or(song.id),
            );
            if let Some(entry) = history.get(&identity) {
                song.play_count = entry.plays;
                song.last_played = entry.last_played;
            }
        }
    }
    check_cancel(token)?;
    Ok(library)
}

fn sample_rate(song: &Song) -> Result<f64> {
    song.sample_rate.ok_or_else(|| {
        Error::Invalid(format!(
            "track {} has positions without a sample rate",
            song.id
        ))
    })
}
fn date(value: Option<&str>) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    Ok(value.map(parse_timestamp).transpose()?.flatten())
}
fn nonnegative<T: TryFrom<i64>>(value: i64, field: &str) -> Result<T> {
    value
        .try_into()
        .map_err(|_| Error::Invalid(format!("invalid {field}: {value}")))
}

fn song_from_record(record: &TrackRecord, base: &Path, options: &ImportOptions) -> Result<Song> {
    let t = &record.track;
    let m = &record.metadata;
    let path = t.path.as_deref().unwrap_or("");
    let path = if options.preserve_paths {
        path.into()
    } else {
        std::path::absolute(base.join(path))?
    };
    let length = t.length.unwrap_or(0);
    if length < 0 {
        return Err(Error::Invalid("negative track duration".into()));
    }
    let rating = nonnegative::<u8>(t.rating.unwrap_or(0), "rating")?;
    if rating > 100 {
        return Err(Error::Invalid("Engine rating exceeds 100".into()));
    }
    let key = t.key.map(|key| nonnegative::<u8>(key, "key")).transpose()?;
    if key.is_some_and(|key| key > 23) {
        return Err(Error::Invalid("Engine key exceeds 23".into()));
    }
    Ok(Song {
        id: t.id,
        title: t.title.clone().unwrap_or_default(),
        artist: t.artist.clone().unwrap_or_default(),
        composer: t.composer.clone().unwrap_or_default(),
        album: t.album.clone().unwrap_or_default(),
        genre: t.genre.clone().unwrap_or_default(),
        file_type: t.file_type.clone().unwrap_or_default(),
        size: nonnegative(t.file_bytes.unwrap_or(0), "file size")?,
        length: length as f64
            / match options.duration_unit {
                DurationUnit::Seconds => 1.0,
                DurationUnit::Milliseconds => 1000.0,
            },
        year: t.year.map(|year| nonnegative(year, "year")).transpose()?,
        bpm: t.bpm.map(|bpm| bpm as f64),
        bpm_analyzed: t.bpm_analyzed,
        date_modified: date(t.last_edit_time.as_deref())?,
        date_added: date(m.date_added.as_deref())?,
        date_created: date(m.date_created.as_deref())?,
        bitrate: nonnegative(t.bitrate.unwrap_or(0), "bitrate")?,
        comment: t.comment.clone().unwrap_or_default(),
        time_last_played: date(m.time_last_played.as_deref())?,
        is_played: m.is_played.unwrap_or(false),
        played_indicator: m.played_indicator.unwrap_or(0),
        rating,
        path,
        remixer: t.remixer.clone().unwrap_or_default(),
        key,
        label: t.label.clone().unwrap_or_default(),
        is_analyzed: t.is_analyzed.unwrap_or(false),
        is_beat_grid_locked: m.is_beat_grid_locked.unwrap_or(false),
        explicit_lyrics: m.explicit_lyrics.unwrap_or(false),
        origin_database_uuid: m.origin_database_uuid.clone(),
        origin_track_id: m.origin_track_id,
        streaming_source: m.streaming_source.clone(),
        uri: m.uri.clone(),
        streaming_flags: m.streaming_flags,
        album_art_id: t.album_art_id,
        album_art_source_hash: t.album_art_source_hash.clone(),
        ..Song::default()
    })
}

fn playlist(
    node: &PlaylistNode,
    database: &Database,
    token: &CancellationToken,
) -> Result<Playlist> {
    check_cancel(token)?;
    Ok(Playlist {
        id: node.id,
        name: node.title.clone(),
        songs: database
            .playlist_tracks(node.id)?
            .iter()
            .map(|track| track.id)
            .collect(),
        children: node
            .children
            .iter()
            .map(|child| playlist(child, database, token))
            .collect::<Result<_>>()?,
    })
}

pub(super) fn grid(
    rate: f64,
    markers: &[engine_library::performance::BeatMarker],
) -> Result<Vec<Marker>> {
    let mut grid = Vec::new();
    for pair in markers.windows(2) {
        let span = pair[1].samples - pair[0].samples;
        let bpm = rate * 60.0 * f64::from(pair[0].beats_to_next) / span;
        if span <= 0.0 || !bpm.is_finite() || bpm <= 0.0 {
            return Err(Error::Invalid(
                "non-increasing or zero-tempo Engine grid".into(),
            ));
        }
        grid.push(Marker {
            start: pair[0].samples / rate,
            bpm,
            beat: pair[0].beat.rem_euclid(4) as u8,
        });
    }
    if let Some(first) = grid.first_mut().filter(|first| first.start < 0.0) {
        let period = 60.0 / first.bpm;
        let skipped = (-first.start / period).ceil();
        if !skipped.is_finite() || skipped >= u64::MAX as f64 {
            return Err(Error::Invalid("Engine grid offset is out of range".into()));
        }
        first.start += skipped * period;
        first.beat = ((u64::from(first.beat) + skipped as u64 % 4) % 4) as u8;
    }
    Ok(grid)
}

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    pub start: f64,
    pub bpm: f64,
    pub beat: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HotCue {
    pub name: String,
    pub offset: f64,
    pub slot: u8,
    pub color: [u8; 3],
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Loop {
    pub name: String,
    pub start: f64,
    pub end: f64,
    pub slot: u8,
    pub color: [u8; 3],
    pub locked: bool,
}

/// Native fields and opaque caches retained when the portable view is edited.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EngineRecord {
    pub record: engine_library::records::TrackRecord,
    pub performance: Option<engine_library::performance::PerformanceData>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Song {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub composer: String,
    pub album: String,
    pub grouping: String,
    pub genre: String,
    pub file_type: String,
    pub size: u64,
    /// Duration in seconds, independent of the storage units used by a source.
    pub length: f64,
    pub track_number: u32,
    pub year: Option<u32>,
    pub bpm: Option<f64>,
    pub bpm_analyzed: Option<f64>,
    pub date_modified: Option<DateTime<Utc>>,
    pub date_added: Option<DateTime<Utc>>,
    pub date_created: Option<DateTime<Utc>>,
    pub bitrate: u32,
    pub sample_rate: Option<f64>,
    pub comment: String,
    pub play_count: u64,
    pub last_played: Option<DateTime<Utc>>,
    pub time_last_played: Option<DateTime<Utc>>,
    pub is_played: bool,
    pub played_indicator: i64,
    pub rating: u8,
    pub path: PathBuf,
    pub remixer: String,
    /// Camelot order: 0=8B, 1=8A, ... 23=7A. None represents an unknown key.
    pub key: Option<u8>,
    pub label: String,
    pub mix: String,
    pub color: Option<[u8; 3]>,
    pub cue: Option<f64>,
    pub grid: Vec<Marker>,
    pub cues: Vec<HotCue>,
    pub loops: Vec<Loop>,
    pub corrupt: bool,
    pub is_analyzed: bool,
    pub is_beat_grid_locked: bool,
    pub explicit_lyrics: bool,
    pub origin_database_uuid: Option<String>,
    pub origin_track_id: Option<i64>,
    pub streaming_source: Option<String>,
    pub uri: Option<String>,
    pub streaming_flags: Option<i64>,
    pub album_art_id: Option<i64>,
    pub album_art_source_hash: Option<String>,
    pub overview_wave_form_data: Option<Vec<u8>>,
    pub track_data: Option<Vec<u8>>,
    pub active_on_load_loops: Option<i64>,
    pub engine: Option<EngineRecord>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub songs: Vec<i64>,
    pub children: Vec<Playlist>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Library {
    pub database_uuid: Option<uuid::Uuid>,
    pub schema_version: Option<[u32; 3]>,
    pub songs: Vec<Song>,
    pub playlists: Vec<Playlist>,
    pub smartlists: Vec<engine_library::records::Smartlist>,
    pub artwork: Vec<engine_library::AlbumArt>,
}

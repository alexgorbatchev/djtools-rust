use super::{Markers, extract_crates, parse_markers_, parse_markers2};
use crate::{Error, Library, Playlist, Result, Song, check_cancel};
use lofty::{
    file::{AudioFile, TaggedFileExt},
    id3::v2::{Frame, GeneralEncapsulatedObject, Id3v2Tag},
    tag::{Accessor, ItemKey, TagType},
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use tokio_util::sync::CancellationToken;

/// Original ID3 frame content, retained independently of the portable marker view.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Geob {
    pub descriptor: String,
    pub mime_type: Option<String>,
    pub file_name: Option<String>,
    pub data: Vec<u8>,
    pub frame: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExtractedSong {
    pub song: Song,
    pub objects: Vec<Geob>,
    pub markers: Markers,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ImportWarning {
    pub path: PathBuf,
    pub message: String,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ImportReport {
    pub library: Library,
    pub warnings: Vec<ImportWarning>,
}

fn audio_error(error: impl std::fmt::Display) -> Error {
    Error::Invalid(format!("audio metadata: {error}"))
}
pub fn extract_song(path: impl AsRef<Path>) -> Result<ExtractedSong> {
    let path = path.as_ref();
    let mut tagged = lofty::probe::Probe::open(path)
        .map_err(audio_error)?
        .guess_file_type()
        .map_err(audio_error)?
        .read()
        .map_err(audio_error)?;
    let properties = tagged.properties();
    let mut song = Song {
        path: path.to_owned(),
        length: properties.duration().as_secs_f64(),
        bitrate: properties.audio_bitrate().unwrap_or(0),
        sample_rate: properties.sample_rate().map(f64::from),
        size: std::fs::metadata(path)?.len(),
        ..Song::default()
    };
    if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
        song.title = tag.title().unwrap_or_default().into_owned();
        song.artist = tag.artist().unwrap_or_default().into_owned();
        song.album = tag.album().unwrap_or_default().into_owned();
        song.genre = tag.genre().unwrap_or_default().into_owned();
        song.comment = tag.comment().unwrap_or_default().into_owned();
        song.year = tag.date().map(|date| u32::from(date.year));
        song.track_number = tag.track().unwrap_or(0);
        song.composer = tag.get_string(ItemKey::Composer).unwrap_or_default().into();
        if let Some(value) = tag
            .get_string(ItemKey::Bpm)
            .or_else(|| tag.get_string(ItemKey::IntegerBpm))
        {
            let bpm: f64 = value.parse().map_err(|_| audio_error("invalid BPM"))?;
            if !bpm.is_finite() || bpm <= 0.0 {
                return Err(audio_error("invalid BPM"));
            }
            song.bpm = Some(bpm);
        }
        song.key = tag
            .get_string(ItemKey::InitialKey)
            .and_then(crate::rbxml::musical_key);
    }
    fallback(&mut song);
    let mut objects = Vec::new();
    let mut markers = Markers::default();
    let mut legacy = None;
    if let Some(tag) = tagged.remove(TagType::Id3v2) {
        // Lofty's native roundtrip restores format-specific GEOB frames from the companion tag.
        let tag = Id3v2Tag::from(tag);
        for frame in tag.iter() {
            if frame.id_str() != "GEOB" {
                continue;
            }
            let Frame::Binary(binary) = frame else {
                continue;
            };
            let object = GeneralEncapsulatedObject::parse(&binary.data, frame.flags())
                .map_err(audio_error)?;
            let descriptor = object.descriptor.unwrap_or_default();
            match descriptor.as_str() {
                "Serato Markers2" => markers = parse_markers2(&object.data)?,
                "Serato Markers_" => legacy = Some(parse_markers_(&object.data)?),
                _ => {}
            }
            objects.push(Geob {
                descriptor,
                mime_type: object.mime_type,
                file_name: object.file_name,
                data: object.data,
                frame: binary.data.to_vec(),
            });
        }
    }
    if let Some(old) = legacy {
        markers.merge_legacy(old);
    }
    song.cues = markers.cues.clone();
    song.loops = markers.loops.clone();
    song.color = markers.color;
    song.is_beat_grid_locked = markers.beat_grid_locked.unwrap_or(false);
    objects.sort_by(|a, b| a.descriptor.cmp(&b.descriptor));
    Ok(ExtractedSong {
        song,
        objects,
        markers,
    })
}
fn fallback(song: &mut Song) {
    if song.title.is_empty() {
        song.title = song
            .path
            .file_stem()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
    }
    song.file_type = song
        .path
        .extension()
        .map(|extension| extension.to_string_lossy().into_owned())
        .unwrap_or_default();
}
/// Preserve crate membership even when audio is absent; report every extraction failure.
pub fn import(path: impl AsRef<Path>, token: &CancellationToken) -> Result<ImportReport> {
    check_cancel(token)?;
    let crates = extract_crates(path)?;
    let mut paths = BTreeMap::new();
    let mut library = Library::default();
    let mut warnings = Vec::new();
    for (index, value) in crates.into_iter().enumerate() {
        check_cancel(token)?;
        let mut songs = Vec::new();
        for path in value.paths {
            check_cancel(token)?;
            let id = if let Some(id) = paths.get(&path) {
                *id
            } else {
                let id = i64::try_from(library.songs.len())
                    .map_err(|_| Error::Invalid("song identity overflow".into()))?
                    + 1;
                let mut song = match extract_song(&path) {
                    Ok(extracted) => extracted.song,
                    Err(error) => {
                        warnings.push(ImportWarning {
                            path: path.clone(),
                            message: error.to_string(),
                        });
                        let mut song = Song {
                            path: path.clone(),
                            ..Song::default()
                        };
                        fallback(&mut song);
                        song
                    }
                };
                song.id = id;
                library.songs.push(song);
                paths.insert(path, id);
                id
            };
            songs.push(id);
        }
        library.playlists.push(Playlist {
            id: index as i64 + 1,
            name: value.filename,
            songs,
            children: vec![],
        });
    }
    library.validate()?;
    Ok(ImportReport { library, warnings })
}

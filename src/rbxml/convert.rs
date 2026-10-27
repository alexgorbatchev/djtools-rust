use super::{Document, ExportOptions, Node, NodeTrack, PositionMark, Tempo, Track};
use crate::{Error, HotCue, Library, Loop, Marker, Playlist, Result, Song};
use std::collections::BTreeMap;

const CAMELOT: [&str; 24] = [
    "8B", "8A", "9B", "9A", "10B", "10A", "11B", "11A", "12B", "12A", "1B", "1A", "2B", "2A", "3B",
    "3A", "4B", "4A", "5B", "5A", "6B", "6A", "7B", "7A",
];
const NOTES: [&str; 24] = [
    "C", "Am", "G", "Em", "D", "Bm", "A", "F#m", "E", "C#m", "B", "G#m", "F#", "D#m", "Db", "Bbm",
    "Ab", "Fm", "Eb", "Cm", "Bb", "Gm", "F", "Dm",
];
pub(crate) fn musical_key(value: &str) -> Option<u8> {
    CAMELOT
        .iter()
        .position(|key| *key == value)
        .or_else(|| NOTES.iter().position(|key| *key == value))
        .map(|key| key as u8)
}
fn date(value: &str) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    Ok(engine_library::records::parse_timestamp(value)?)
}
fn date_string(value: Option<chrono::DateTime<chrono::Utc>>, options: &ExportOptions) -> String {
    value
        .map(|date| {
            if options.use_utc {
                date.format("%Y-%m-%d").to_string()
            } else {
                date.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d")
                    .to_string()
            }
        })
        .unwrap_or_default()
}

pub(super) fn to_library(document: &Document) -> Result<Library> {
    let mut library = Library::default();
    let mut locations = BTreeMap::new();
    for track in &document.collection.tracks {
        let url = url::Url::parse(&track.location)
            .map_err(|error| Error::Invalid(format!("invalid location: {error}")))?;
        let path = url
            .to_file_path()
            .map_err(|_| Error::Invalid("track location is not a local file URL".into()))?;
        if locations.insert(track.location.clone(), track.id).is_some() {
            return Err(Error::Invalid("duplicate track location".into()));
        }
        let key = musical_key(&track.tonality);
        let rating = match track.rating {
            0 => 0,
            51 => 20,
            102 => 40,
            153 => 60,
            204 => 80,
            255 => 100,
            1..=5 => track.rating as u8 * 20,
            6..=100 => track.rating as u8,
            _ => return Err(Error::Invalid("invalid Rekordbox rating".into())),
        };
        let color = if track.color.is_empty() {
            None
        } else {
            let value =
                u32::from_str_radix(track.color.strip_prefix("0x").unwrap_or(&track.color), 16)
                    .map_err(|_| Error::Invalid("invalid track color".into()))?;
            if value > 0xffffff {
                return Err(Error::Invalid("track color exceeds RGB".into()));
            }
            Some([(value >> 16) as u8, (value >> 8) as u8, value as u8])
        };
        let mut song = Song {
            id: track.id,
            title: track.name.clone(),
            artist: track.artist.clone(),
            composer: track.composer.clone(),
            album: track.album.clone(),
            grouping: track.grouping.clone(),
            genre: track.genre.clone(),
            file_type: track.kind.clone(),
            size: track.size,
            length: track.total_time,
            track_number: track.track_number,
            year: (track.year != 0).then_some(track.year),
            bpm: (track.average_bpm != 0.0).then_some(track.average_bpm),
            date_modified: date(&track.date_modified)?,
            date_added: date(&track.date_added)?,
            bitrate: track.bitrate,
            sample_rate: (track.sample_rate != 0.0).then_some(track.sample_rate),
            comment: track.comments.clone(),
            play_count: track.play_count,
            last_played: date(&track.last_played)?,
            rating,
            path,
            remixer: track.remixer.clone(),
            key,
            label: track.label.clone(),
            mix: track.mix.clone(),
            color,
            ..Song::default()
        };
        for tempo in &track.tempos {
            if !tempo.meter.is_empty() && tempo.meter != "4/4" {
                return Err(Error::Invalid(
                    "portable beat grids require 4/4 meter".into(),
                ));
            }
            if tempo.beat == 0 || tempo.beat > 4 {
                return Err(Error::Invalid(
                    "beat number is outside portable 4/4 grid".into(),
                ));
            }
            song.grid.push(Marker {
                start: tempo.start,
                bpm: tempo.bpm,
                beat: tempo.beat - 1,
            });
        }
        for mark in &track.position_marks {
            if mark.kind == 0 && mark.num == -1 {
                song.cue = Some(mark.start);
                continue;
            }
            if mark.num < 0 {
                continue;
            }
            let slot = u8::try_from(mark.num)
                .ok()
                .and_then(|slot| slot.checked_add(1))
                .ok_or_else(|| Error::Invalid("cue slot exceeds portable limit".into()))?;
            match mark.kind {
                0 => song.cues.push(HotCue {
                    name: mark.name.clone(),
                    offset: mark.start,
                    slot,
                    color: [0; 3],
                }),
                4 => song.loops.push(Loop {
                    name: mark.name.clone(),
                    start: mark.start,
                    end: mark.end,
                    slot,
                    color: [0; 3],
                    locked: false,
                }),
                _ => {}
            }
        }
        library.songs.push(song);
    }
    if document.playlists.node.node_type != 0 {
        return Err(Error::Invalid("playlist root must be a folder".into()));
    }
    library.playlists = nodes(&document.playlists.node.children, &locations, &mut 1, 0)?;
    library.validate()?;
    Ok(library)
}
fn nodes(
    children: &[Node],
    locations: &BTreeMap<String, i64>,
    next: &mut i64,
    depth: usize,
) -> Result<Vec<Playlist>> {
    if depth > 128 {
        return Err(Error::Invalid(
            "playlist hierarchy exceeds 128 levels".into(),
        ));
    }
    children
        .iter()
        .map(|node| {
            if node.node_type > 1 || node.key_type > 1 {
                return Err(Error::Invalid("unknown playlist node or key type".into()));
            }
            let id = *next;
            *next += 1;
            let songs = node
                .tracks
                .iter()
                .map(|track| {
                    if node.key_type == 1 {
                        locations.get(&track.key).copied().ok_or_else(|| {
                            Error::Invalid("absent location playlist reference".into())
                        })
                    } else {
                        track.key.parse().map_err(|_| {
                            Error::Invalid("invalid track ID playlist reference".into())
                        })
                    }
                })
                .collect::<Result<_>>()?;
            Ok(Playlist {
                id,
                name: node.name.clone(),
                songs,
                children: nodes(&node.children, locations, next, depth + 1)?,
            })
        })
        .collect()
}

pub(super) fn from_library(library: &Library, options: &ExportOptions) -> Result<Document> {
    library.validate()?;
    let mut document = Document::new();
    for song in &library.songs {
        let absolute = std::path::absolute(&song.path)?;
        let mut uri = url::Url::from_file_path(&absolute)
            .map_err(|_| Error::Invalid("path cannot be represented as a file URL".into()))?;
        uri.set_host(Some("localhost"))
            .map_err(|_| Error::Invalid("cannot set file URI host".into()))?;
        let mut track = Track {
            id: song.id,
            name: song.title.clone(),
            artist: song.artist.clone(),
            composer: song.composer.clone(),
            album: song.album.clone(),
            grouping: song.grouping.clone(),
            genre: song.genre.clone(),
            kind: song.file_type.clone(),
            size: song.size,
            total_time: song.length,
            track_number: song.track_number,
            year: song.year.unwrap_or(0),
            average_bpm: song
                .bpm
                .filter(|bpm| *bpm > 0.0)
                .or(song.bpm_analyzed)
                .or_else(|| song.grid.first().map(|marker| marker.bpm))
                .unwrap_or(0.0),
            date_modified: date_string(song.date_modified, options),
            date_added: date_string(song.date_added, options),
            bitrate: song.bitrate,
            sample_rate: song.sample_rate.unwrap_or(0.0),
            comments: song.comment.clone(),
            play_count: song.play_count,
            last_played: date_string(song.last_played, options),
            rating: if song.rating % 20 == 0 {
                u16::from(song.rating / 20) * 51
            } else {
                u16::from(song.rating)
            },
            location: uri.to_string(),
            remixer: song.remixer.clone(),
            tonality: song
                .key
                .map(|key| CAMELOT[usize::from(key)].to_owned())
                .unwrap_or_default(),
            label: song.label.clone(),
            mix: song.mix.clone(),
            color: song
                .color
                .map(|[r, g, b]| format!("0x{r:02X}{g:02X}{b:02X}"))
                .unwrap_or_default(),
            ..Track::default()
        };
        if let Some(cue) = song.cue {
            track.position_marks.push(PositionMark {
                kind: 0,
                start: cue,
                num: -1,
                ..PositionMark::default()
            });
        }
        for cue in &song.cues {
            track.position_marks.push(PositionMark {
                name: cue.name.clone(),
                kind: 0,
                start: cue.offset,
                num: i32::from(cue.slot) - 1,
                ..PositionMark::default()
            });
        }
        for value in &song.loops {
            track.position_marks.push(PositionMark {
                name: value.name.clone(),
                kind: 4,
                start: value.start,
                end: value.end,
                num: i32::from(value.slot) - 1,
            });
        }
        track.tempos = song
            .grid
            .iter()
            .map(|marker| Tempo {
                start: marker.start,
                bpm: marker.bpm,
                meter: "4/4".into(),
                beat: marker.beat + 1,
            })
            .collect();
        document.add_track(track);
    }
    document.playlists.node.children = library.playlists.iter().map(export_node).collect();
    Ok(document)
}
fn export_node(list: &Playlist) -> Node {
    let tracks = list
        .songs
        .iter()
        .map(|id| NodeTrack {
            key: id.to_string(),
        })
        .collect();
    if list.children.is_empty() {
        return Node {
            node_type: 1,
            name: list.name.clone(),
            tracks,
            ..Node::default()
        };
    }
    let mut children = Vec::new();
    if !list.songs.is_empty() {
        children.push(Node {
            node_type: 1,
            name: list.name.clone(),
            tracks,
            ..Node::default()
        });
    }
    children.extend(list.children.iter().map(export_node));
    Node {
        name: list.name.clone(),
        children,
        ..Node::default()
    }
}

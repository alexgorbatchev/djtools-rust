`djtools` lets Rust applications exchange DJ collections between Engine DJ databases, Rekordbox XML, Serato crates, and a portable in-memory library. It preserves playlist membership, track metadata, and supported cue and loop positions without requiring a command-line program.

# What It Does

- **Engine collections:** Reads and writes the `m.db` and `hm.db` pair, including database identity, artwork records, smartlists, play history, and native analysis records.
- **Rekordbox interchange:** Reads and writes documented XML metadata, nested playlists, beat grids, memory cues, hot cues, and loops.
- **Serato extraction:** Reads and writes crate files and extracts audio tags plus ID3 `Serato Markers2` and `Serato Markers_` data.
- **Portable editing:** Loads and saves JSON, looks up tracks and playlists, sorts tracks and slots, and removes tracks together with their nested playlist references.

# How It Works

1. Import a collection or construct a `Library` containing songs and nested playlists.
2. Inspect or edit the library using native Rust values. Positions and durations use seconds; cue and loop slots start at one.
3. Validate the library and export the chosen format. Inspect returned errors and Serato extraction warnings before using the output.

# How it Really Works

1. Calls block the calling thread and read local files only. Engine and Serato collection operations check the supplied cancellation token between records; an individual SQLite query or audio-tag read completes before the next check. There is no background work, network access, or process-global collection cache.
2. Engine import prefers `Database2/m.db`, falling back to `m.db` in the supplied directory. By default, relative audio paths resolve against that directory. Missing history is allowed; malformed existing databases, analysis blobs, and orphan playlist parents return errors. Tracks without analysis remain valid.
3. Engine import retains complete nullable records and opaque performance bytes alongside the portable view. Unchanged native dates retain their original spelling, and unnamed cues at sample zero remain occupied. Altering a native beat-data sample rate is rejected because original positions and opaque caches require reanalysis. Exports preserve original grids while editing adjusted grids.
4. Engine export stages both databases before publication and leaves unrelated files intact. Existing databases require explicit `overwrite`; SQLite sidecars cause refusal. Publication failures attempt to restore the original pair. `Error::Publication` retains rollback failures and a recovery directory when restoration is incomplete. Two separate files do not form a crash-atomic transaction: close other collection users and keep a backup before replacement.
5. Rekordbox conversion uses local file URLs without truncating long paths, accepts ID- and location-keyed playlists, and preserves `DateModified` and memory cues. Unknown musical keys stay absent. Cue times are not shifted for MP3 files. The portable grid represents 4/4; other explicit meters return an error. XML dates export in UTC by default.
6. Serato import deduplicates audio paths while retaining crate membership order. A missing or unreadable audio file keeps its playlist entry and produces an explicit warning with filename-derived metadata. Native ID3 decoding supports Unicode GEOB descriptions; legacy markers override their corresponding Markers2 positions. Extracted unknown marker entries and original GEOB content remain available to the caller. Keep Lofty's default format-specific-item preservation enabled when using its global options.
7. JSON, XML, and crate saves publish a unique sibling temporary file after writing and syncing it. Caller-owned in-memory values have no implicit synchronization; use separate values or your own locking when editing concurrently. No file is modified merely by importing this package.

# Prerequisites

- Rust 1.89 or newer.
- An Engine DJ collection, Rekordbox XML document, or Serato crate directory when importing those formats.

# Installation

This Rust package is unpublished. From a consumer beside this local checkout:

```sh
cargo add djtools --path ../djtools
```

Cargo fetches `engine-library` and its transitive dependencies from pinned GitHub revisions. Contributor commands are in [AGENTS.md](AGENTS.md).

# Quick Start

This creates and parses a Rekordbox document entirely in memory:

```rust
use djtools::{Library, Song, rbxml};

fn main() -> djtools::Result<()> {
    let library = Library {
        songs: vec![Song {
            id: 1,
            title: "Example track".into(),
            path: std::env::current_dir()?.join("Music/example.mp3"),
            length: 180.0,
            ..Song::default()
        }],
        ..Library::default()
    };
    let document = rbxml::Document::from_library(&library, &rbxml::ExportOptions::default())?;
    let restored = rbxml::Document::from_xml(&document.to_xml()?)?.to_library()?;
    assert_eq!(restored.songs[0].title, "Example track");
    Ok(())
}
```

# API

| Entry point | Behavior |
| :--- | :--- |
| `Library::load`, `save`, `validate` | Load or atomically replace portable JSON; reject invalid identities, memberships, numeric values, and cue/loop slots. |
| `Library::song_by_id`, `song_by_path`, `song_by_title` | Look up a song; title lookup ignores Unicode letter case. |
| `Library::playlist_by_id`, `playlist_by_name` | Search nested playlists. |
| `Library::sort`, `add_song`, `remove_song`, `remove_corrupt` | Edit tracks and maintain nested memberships on removal; validate before publication. |
| `format_rgb`, `parse_rgb` | Format or strictly parse `#RRGGBB`. |
| `engine::import`, `export` | Read or publish an Engine collection with explicit options and a cancellation token. |
| `rbxml::Document::{new,open,from_xml,to_xml,save}` | Read, construct, validate, and serialize the supported native XML fields. |
| `rbxml::Document::{to_library,from_library}` | Convert between supported XML metadata and the portable library. |
| `rbxml::Document::{track_by_id,track_by_location,node_by_name,add_track,remove_track,sort}` | Inspect and edit a document, including recursive membership removal. |
| `rbxml::import`, `export` | Convert a file directly to or from a library. |
| `serato::Crate::{read,write}`, `decode_crate`, `encode_crate`, `extract_crates` | Read, write, or discover strict UTF-16BE crates. |
| `serato::parse_markers2`, `parse_markers_` | Decode native binary marker positions, colors, and locks. |
| `serato::extract_song` | Read basic audio tags and original ID3 GEOB objects without changing the audio. |
| `serato::import` | Read crates and return a library plus per-file extraction warnings. |

`Song::engine` retains native records for Engine round trips. Use `engine-library` directly for granular database CRUD. All fallible APIs return `djtools::Result`; generate local API documentation using [AGENTS.md](AGENTS.md).

# Configuration

| Option | Default | Meaning |
| :--- | :--- | :--- |
| Engine `duration_unit` | `DurationUnit::Seconds` | Explicit native integer duration units, matching the inspected djtools source. Choose `Milliseconds` for collections known to use the documented millisecond schema; units are never inferred. |
| Engine `fallback_sample_rate` | `44100.0` | Position conversion when no beat-data sample rate exists. |
| Engine `decompression_limit` | 64 MiB | Maximum decoded size for each compressed analysis blob. |
| Engine import `preserve_paths` | `false` | Retain stored paths instead of resolving against the collection directory. |
| Engine import `original_grids`, `original_cues` | `false` | Select original rather than adjusted analysis positions. |
| Engine export `overwrite` | `false` | Explicitly allow replacement of the closed database pair. |
| XML export `use_utc` | `true` | Emit date-only fields in UTC; `false` uses the local timezone. |

# Format Boundaries

Engine exports support eight hot cue and eight loop slots. A portable locked loop returns an error because its Engine encoding is undocumented. Grid segments must span a positive integral number of beats. Native integer BPM and duration fields retain their integer representation; fractional durations require the explicit millisecond unit. Native opaque waveform and track caches are preserved, not regenerated after edits.

Rekordbox XML cannot carry every Engine-only field; use native portable JSON to retain that data between edits. XML metadata outside the declared document model is not retained. Serato audio-marker extraction covers ID3 GEOB frames, not proprietary databases or other audio-container marker layouts. Crate writing is supported; writing Serato audio markers is not exposed.

Local tests exercise real SQLite databases, rollback and recovery paths, XML documents, generated MP3 files with Unicode ID3 frames, and malformed binary input. Engine DJ, Rekordbox, and Serato desktop/device acceptance requires separate validation. The source checkout contains no implemented command-line entrypoint; this package exposes a library API.

Format references: [Rekordbox XML specification](https://cdn.rekordbox.com/files/20200410160904/xml_format_list.pdf), [Serato marker layout research](https://github.com/Holzhaus/serato-tags/tree/main/docs). Binary fixtures here are generated from the field layouts; no reference audio assets are included.

# License

[MIT](LICENSE), copyright 2025 Nate Randa and 2026 Alex Gorbatchev.

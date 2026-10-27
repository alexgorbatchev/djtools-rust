use super::{export_performance::performance, export_record::record, publication::publish};
use crate::{Error, Library, Playlist, Result, check_cancel};
use engine_library::{Database, model::DurationUnit, records::Information};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug)]
pub struct ExportOptions {
    pub overwrite: bool,
    pub duration_unit: DurationUnit,
    pub fallback_sample_rate: f64,
    pub decompression_limit: usize,
}
impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            overwrite: false,
            duration_unit: DurationUnit::Seconds,
            fallback_sample_rate: 44100.0,
            decompression_limit: 64 * 1024 * 1024,
        }
    }
}

/// Stage the complete database pair before publication; unrelated collection files remain intact.
pub fn export(
    library: &Library,
    path: impl AsRef<Path>,
    options: &ExportOptions,
    token: &CancellationToken,
) -> Result<()> {
    check_cancel(token)?;
    library.validate()?;
    let destination = path.as_ref().join("Database2");
    std::fs::create_dir_all(&destination)?;
    for name in ["m.db", "hm.db"] {
        let target = destination.join(name);
        if target.try_exists()? {
            if !options.overwrite {
                return Err(Error::Invalid(format!(
                    "{} already exists",
                    target.display()
                )));
            }
            if !std::fs::symlink_metadata(&target)?.file_type().is_file() {
                return Err(Error::Invalid(
                    "database destination is not a regular file".into(),
                ));
            }
        }
        for suffix in ["-wal", "-shm", "-journal"] {
            if destination.join(format!("{name}{suffix}")).try_exists()? {
                return Err(Error::Invalid(
                    "database has live SQLite sidecars; close other users before export".into(),
                ));
            }
        }
    }
    let staging = tempfile::Builder::new()
        .prefix(".djtools-stage-")
        .tempdir_in(&destination)?;
    let database = Database::create(staging.path().join("m.db"))?;
    let history = Database::create_history(staging.path().join("hm.db"))?;
    let current = database.information()?.expect("new database information");
    let information = Information {
        uuid: library.database_uuid.unwrap_or(current.uuid),
        schema_version: library.schema_version.unwrap_or(current.schema_version),
        ..current
    };
    database.put_information(&information)?;
    history.put_information(&information)?;
    for art in &library.artwork {
        check_cancel(token)?;
        database.put_artwork(art)?;
    }
    let mut used: BTreeSet<_> = library
        .songs
        .iter()
        .filter(|song| song.id > 0)
        .map(|song| song.id)
        .collect();
    let mut identities = BTreeMap::new();
    for song in &library.songs {
        check_cancel(token)?;
        let id = if song.id == 0 {
            allocate(&mut used)?
        } else {
            song.id
        };
        identities.insert(song.id, id);
        let record = record(song, id, options)?;
        database.put_track_record(&record)?;
        if let Some(performance) = performance(song, id, options)? {
            database.put_performance(&performance)?;
        }
        if song.play_count > 0 {
            history.put_track_record(&record)?;
            for index in 0..song.play_count {
                check_cancel(token)?;
                engine_library::history::append(
                    history.connection(),
                    id,
                    if index + 1 == song.play_count {
                        song.last_played
                    } else {
                        None
                    },
                )?;
            }
        }
    }
    let mut used_lists = BTreeSet::new();
    playlist_ids(&library.playlists, &mut used_lists);
    lists(
        &database,
        &library.playlists,
        0,
        &mut used_lists,
        &identities,
        token,
    )?;
    for smartlist in &library.smartlists {
        check_cancel(token)?;
        database.put_smartlist(smartlist)?;
    }
    drop(database);
    drop(history);
    for name in ["m.db", "hm.db"] {
        std::fs::File::open(staging.path().join(name))?.sync_all()?;
    }
    check_cancel(token)?;
    publish(staging.path(), &destination)
}

fn allocate(used: &mut BTreeSet<i64>) -> Result<i64> {
    let mut id = 1;
    while used.contains(&id) {
        id = id
            .checked_add(1)
            .ok_or_else(|| Error::Invalid("identity space exhausted".into()))?;
    }
    used.insert(id);
    Ok(id)
}
fn playlist_ids(lists: &[Playlist], used: &mut BTreeSet<i64>) {
    for list in lists {
        if list.id > 0 {
            used.insert(list.id);
        }
        playlist_ids(&list.children, used);
    }
}
fn lists(
    database: &Database,
    playlists: &[Playlist],
    parent: i64,
    used: &mut BTreeSet<i64>,
    songs: &BTreeMap<i64, i64>,
    token: &CancellationToken,
) -> Result<()> {
    for list in playlists {
        check_cancel(token)?;
        let id = if list.id == 0 {
            allocate(used)?
        } else {
            list.id
        };
        database.insert_playlist_record(&engine_library::Playlist {
            id,
            title: list.name.clone(),
            parent_list_id: parent,
            is_folder: None,
        })?;
        for song in &list.songs {
            database.add_membership(id, songs[song])?;
        }
        lists(database, &list.children, id, used, songs, token)?;
    }
    Ok(())
}

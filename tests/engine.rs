use djtools::engine::{self, ImportOptions};
use engine_library::{
    Database, Track,
    performance::{BeatData, PerformanceData, q_compress},
    records::{TrackMetadata, TrackRecord},
};
use tokio_util::sync::CancellationToken;

fn database() -> (tempfile::TempDir, Database) {
    std::fs::create_dir_all(".tmp").unwrap();
    let dir = tempfile::tempdir_in(".tmp").unwrap();
    std::fs::create_dir(dir.path().join("Database2")).unwrap();
    let db = Database::create(dir.path().join("Database2/m.db")).unwrap();
    (dir, db)
}

#[test]
fn import_uses_local_identity_retains_unanalyzed_tracks_and_reports_orphan_parents() {
    let (dir, db) = database();
    for (id, origin) in [(1, "source-a"), (2, "source-b")] {
        db.put_track_record(&TrackRecord {
            track: Track {
                id,
                title: Some(format!("Song{id}")),
                path: Some(format!("Music/{id}.mp3")),
                length: Some(120),
                ..Track::default()
            },
            metadata: TrackMetadata {
                origin_track_id: Some(7),
                origin_database_uuid: Some(origin.into()),
                ..TrackMetadata::default()
            },
        })
        .unwrap();
    }
    let list = db.create_playlist("Tracks", 0, false).unwrap();
    db.add_membership(list.id, 2).unwrap();
    db.add_membership(list.id, 1).unwrap();
    let library = engine::import(
        dir.path(),
        &ImportOptions::default(),
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(
        library.songs.iter().map(|s| s.id).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(library.playlists[0].songs, vec![2, 1]);
    assert!(!library.songs[0].corrupt);
    assert!(library.songs[0].grid.is_empty());
    assert!(library.songs[0].path.is_absolute());
    assert!(library.songs[0].date_added.is_none());
    db.connection()
        .execute("UPDATE Playlist SET parentListId=999 WHERE id=?", [list.id])
        .unwrap();
    assert!(
        engine::import(
            dir.path(),
            &ImportOptions::default(),
            &CancellationToken::new()
        )
        .is_err()
    );
}

#[test]
fn native_analysis_grid_math_original_selection_and_corruption_are_checked() {
    let (dir, db) = database();
    db.put_track_record(&TrackRecord {
        track: Track {
            id: 1,
            ..Track::default()
        },
        ..TrackRecord::default()
    })
    .unwrap();
    let mut beats = BeatData::decode(include_bytes!(
        "../../engine-cli/crates/engine-library/tests/fixtures/beat-data.bin"
    ))
    .unwrap();
    beats.sample_rate = 44100.0;
    beats.original[0].samples = 0.0;
    beats.original[0].beat = 0;
    beats.original[0].beats_to_next = 4;
    beats.original[1].samples = 88200.0;
    beats.adjusted = beats.original.clone();
    beats.adjusted[0].samples = -22050.0;
    beats.adjusted[1].samples = 66150.0;
    db.put_performance(&PerformanceData {
        track_id: 1,
        beat_data: Some(q_compress(&beats.encode().unwrap()).unwrap()),
        ..PerformanceData::default()
    })
    .unwrap();
    let library = engine::import(
        dir.path(),
        &ImportOptions::default(),
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(library.songs[0].grid[0].bpm, 120.0);
    assert_eq!(library.songs[0].grid[0].start, 0.0);
    assert_eq!(library.songs[0].grid[0].beat, 1);
    let options = ImportOptions {
        original_grids: true,
        ..ImportOptions::default()
    };
    let original = engine::import(dir.path(), &options, &CancellationToken::new()).unwrap();
    assert_eq!(original.songs[0].grid[0].beat, 0);
    db.put_performance(&PerformanceData {
        track_id: 1,
        beat_data: Some(vec![0; 8]),
        ..PerformanceData::default()
    })
    .unwrap();
    assert!(engine::import(dir.path(), &options, &CancellationToken::new()).is_err());
    let token = CancellationToken::new();
    token.cancel();
    assert!(matches!(
        engine::import(dir.path(), &options, &token),
        Err(djtools::Error::Cancelled)
    ));
}

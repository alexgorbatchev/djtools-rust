use djtools::{
    HotCue, Library, Loop, Marker, Playlist, Song,
    engine::{self, ExportOptions, ImportOptions},
};
use tokio_util::sync::CancellationToken;

#[test]
fn staged_export_retains_identity_order_analysis_history_and_unrelated_files() {
    std::fs::create_dir_all(".tmp").unwrap();
    let dir = tempfile::tempdir_in(".tmp").unwrap();
    let token = CancellationToken::new();
    let source = Library {
        songs: vec![Song {
            id: 7,
            title: "Unicode 音楽".into(),
            path: "Music/a.mp3".into(),
            length: 120.0,
            bpm: Some(120.0),
            sample_rate: Some(44100.0),
            cue: Some(0.0),
            cues: vec![HotCue {
                name: String::new(),
                offset: 0.0,
                slot: 8,
                color: [1, 2, 3],
            }],
            loops: vec![Loop {
                name: "Loop".into(),
                start: 1.0,
                end: 3.0,
                slot: 1,
                color: [4, 5, 6],
                locked: false,
            }],
            grid: vec![Marker {
                start: 0.0,
                bpm: 120.0,
                beat: 0,
            }],
            play_count: 2,
            last_played: chrono::DateTime::from_timestamp(1700000000, 0),
            ..Song::default()
        }],
        playlists: vec![
            Playlist {
                id: 0,
                name: "Allocated".into(),
                songs: vec![7],
                children: vec![],
            },
            Playlist {
                id: 1,
                name: "Explicit".into(),
                songs: vec![],
                children: vec![Playlist {
                    id: 9,
                    name: "Child".into(),
                    songs: vec![7],
                    children: vec![],
                }],
            },
        ],
        ..Library::default()
    };
    engine::export(&source, dir.path(), &ExportOptions::default(), &token).unwrap();
    let options = ImportOptions {
        preserve_paths: true,
        ..ImportOptions::default()
    };
    let library = engine::import(dir.path(), &options, &token).unwrap();
    assert!(library.database_uuid.is_some());
    assert_ne!(library.database_uuid, Some(uuid::Uuid::nil()));
    assert_ne!(library.playlists[0].id, 1);
    assert_eq!(library.playlists[1].id, 1);
    assert_eq!(library.playlists[1].children[0].id, 9);
    assert_eq!(library.playlists[0].songs, [7]);
    let song = &library.songs[0];
    assert_eq!(song.title, source.songs[0].title);
    assert_eq!(song.grid, source.songs[0].grid);
    assert_eq!(song.cue, Some(0.0));
    assert_eq!(song.cues, source.songs[0].cues);
    assert_eq!(song.loops, source.songs[0].loops);
    assert_eq!(song.play_count, 2);
    assert_eq!(song.last_played, source.songs[0].last_played);
    let mut resampled = library.clone();
    resampled.songs[0].sample_rate = Some(48000.0);
    assert!(matches!(
        engine::export(
            &resampled,
            dir.path().join("resampled"),
            &ExportOptions::default(),
            &token
        ),
        Err(djtools::Error::Invalid(_))
    ));
    assert!(engine::export(&source, dir.path(), &ExportOptions::default(), &token).is_err());
    std::fs::write(dir.path().join("Database2/other.db"), b"preserved").unwrap();
    let options = ExportOptions {
        overwrite: true,
        ..ExportOptions::default()
    };
    engine::export(&library, dir.path(), &options, &token).unwrap();
    assert_eq!(
        std::fs::read(dir.path().join("Database2/other.db")).unwrap(),
        b"preserved"
    );
    assert_eq!(
        engine::import(
            dir.path(),
            &ImportOptions {
                preserve_paths: true,
                ..Default::default()
            },
            &token
        )
        .unwrap()
        .songs[0]
            .engine,
        library.songs[0].engine
    );
}

#[test]
fn invalid_export_and_cancellation_preserve_the_existing_database_pair() {
    std::fs::create_dir_all(".tmp").unwrap();
    let dir = tempfile::tempdir_in(".tmp").unwrap();
    let token = CancellationToken::new();
    engine::export(
        &Library::default(),
        dir.path(),
        &ExportOptions::default(),
        &token,
    )
    .unwrap();
    let m = std::fs::read(dir.path().join("Database2/m.db")).unwrap();
    let hm = std::fs::read(dir.path().join("Database2/hm.db")).unwrap();
    let invalid = Library {
        songs: vec![Song {
            id: 1,
            length: f64::NAN,
            ..Song::default()
        }],
        ..Library::default()
    };
    let options = ExportOptions {
        overwrite: true,
        ..Default::default()
    };
    assert!(engine::export(&invalid, dir.path(), &options, &token).is_err());
    token.cancel();
    assert!(matches!(
        engine::export(&Library::default(), dir.path(), &options, &token),
        Err(djtools::Error::Cancelled)
    ));
    assert_eq!(std::fs::read(dir.path().join("Database2/m.db")).unwrap(), m);
    assert_eq!(
        std::fs::read(dir.path().join("Database2/hm.db")).unwrap(),
        hm
    );
}

#[test]
fn unchanged_native_timestamp_spelling_cue_alpha_and_opaque_bytes_survive_reexport() {
    use engine_library::{
        Database,
        performance::{Cue, CueData, PerformanceData, q_compress},
        records::TrackRecord,
    };
    std::fs::create_dir_all(".tmp").unwrap();
    let dir = tempfile::tempdir_in(".tmp").unwrap();
    let db = Database::create(dir.path().join("m.db")).unwrap();
    let mut record = TrackRecord::default();
    record.track.id = 3;
    record.track.last_edit_time = Some("1700000000".into());
    record.metadata.date_added = Some("2023-11-14 22:13:20".into());
    record.metadata.date_created = Some("".into());
    record.metadata.time_last_played = Some("2023-11-14T22:13:20+00:00".into());
    db.put_track_record(&record).unwrap();
    let mut cues = CueData {
        original: 0.0,
        modified: 0.0,
        trailing: vec![8, 7, 6],
        ..Default::default()
    };
    cues.slots[0] = Some(Cue {
        name: "".into(),
        samples: 0.0,
        color: [42, 1, 2, 3],
    });
    let raw = PerformanceData {
        track_id: 3,
        quick_cues: Some(q_compress(&cues.encode().unwrap()).unwrap()),
        track_data: Some(vec![1, 2]),
        overview_wave_form_data: Some(vec![9, 8]),
        ..Default::default()
    };
    db.put_performance(&raw).unwrap();
    drop(db);
    let token = CancellationToken::new();
    let options = ImportOptions {
        preserve_paths: true,
        ..Default::default()
    };
    let library = engine::import(dir.path(), &options, &token).unwrap();
    let export_dir = dir.path().join("out");
    engine::export(&library, &export_dir, &ExportOptions::default(), &token).unwrap();
    let imported = engine::import(&export_dir, &options, &token).unwrap();
    assert_eq!(imported.songs[0].engine.as_ref().unwrap().record, record);
    assert_eq!(
        imported.songs[0]
            .engine
            .as_ref()
            .unwrap()
            .performance
            .as_ref()
            .unwrap()
            .quick_cues,
        raw.quick_cues
    );
}

#[test]
fn allocated_track_identity_millisecond_units_artwork_and_smartlists_roundtrip() {
    use engine_library::{AlbumArt, model::DurationUnit, records::Smartlist};
    std::fs::create_dir_all(".tmp").unwrap();
    let dir = tempfile::tempdir_in(".tmp").unwrap();
    let token = CancellationToken::new();
    let source = Library {
        songs: vec![
            Song {
                id: 0,
                title: "Allocated".into(),
                length: 1.234,
                path: "a.flac".into(),
                ..Default::default()
            },
            Song {
                id: 1,
                title: "Reserved".into(),
                ..Default::default()
            },
        ],
        playlists: vec![Playlist {
            id: 0,
            name: "Allocated list".into(),
            songs: vec![0, 1],
            children: vec![],
        }],
        artwork: vec![AlbumArt {
            id: 3,
            hash: Some("hash".into()),
            data: vec![1, 2, 3],
        }],
        smartlists: vec![Smartlist {
            list_uuid: uuid::Uuid::new_v4().to_string(),
            title: Some("Smart".into()),
            rules: Some("{}".into()),
            ..Default::default()
        }],
        ..Default::default()
    };
    engine::export(
        &source,
        dir.path(),
        &ExportOptions {
            duration_unit: DurationUnit::Milliseconds,
            ..Default::default()
        },
        &token,
    )
    .unwrap();
    let imported = engine::import(
        dir.path(),
        &ImportOptions {
            duration_unit: DurationUnit::Milliseconds,
            preserve_paths: true,
            ..Default::default()
        },
        &token,
    )
    .unwrap();
    let allocated = imported.song_by_title("Allocated").unwrap();
    assert_eq!(allocated.id, 2);
    assert_eq!(allocated.length, 1.234);
    assert_eq!(imported.playlists[0].songs, [2, 1]);
    assert_eq!(imported.artwork, source.artwork);
    assert_eq!(imported.smartlists, source.smartlists);
}

#[test]
fn unsupported_performance_and_live_sidecars_preserve_the_existing_database_pair() {
    std::fs::create_dir_all(".tmp").unwrap();
    let dir = tempfile::tempdir_in(".tmp").unwrap();
    let token = CancellationToken::new();
    engine::export(
        &Library::default(),
        dir.path(),
        &ExportOptions::default(),
        &token,
    )
    .unwrap();
    let main = dir.path().join("Database2/m.db");
    let history = dir.path().join("Database2/hm.db");
    let original = [
        std::fs::read(&main).unwrap(),
        std::fs::read(&history).unwrap(),
    ];
    let options = ExportOptions {
        overwrite: true,
        ..Default::default()
    };
    for song in [
        Song {
            id: 1,
            cues: vec![HotCue {
                slot: 9,
                ..Default::default()
            }],
            ..Default::default()
        },
        Song {
            id: 1,
            loops: vec![Loop {
                slot: 9,
                ..Default::default()
            }],
            ..Default::default()
        },
        Song {
            id: 1,
            loops: vec![Loop {
                slot: 1,
                locked: true,
                ..Default::default()
            }],
            ..Default::default()
        },
        Song {
            id: 1,
            grid: vec![
                Marker {
                    start: 0.0,
                    bpm: 120.0,
                    beat: 0,
                },
                Marker {
                    start: 0.3,
                    bpm: 120.0,
                    beat: 1,
                },
            ],
            ..Default::default()
        },
    ] {
        let library = Library {
            songs: vec![song],
            ..Default::default()
        };
        assert!(engine::export(&library, dir.path(), &options, &token).is_err());
        assert_eq!(std::fs::read(&main).unwrap(), original[0]);
        assert_eq!(std::fs::read(&history).unwrap(), original[1]);
    }
    std::fs::write(dir.path().join("Database2/m.db-wal"), b"active").unwrap();
    assert!(engine::export(&Library::default(), dir.path(), &options, &token).is_err());
    assert_eq!(std::fs::read(&main).unwrap(), original[0]);
}

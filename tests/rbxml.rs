use djtools::{
    HotCue, Library, Marker, Playlist, Song,
    rbxml::{Document, ExportOptions, Node, NodeTrack},
};

#[test]
fn xml_roundtrip_uses_native_uri_dates_memory_cues_and_no_mp3_offset_drift() {
    std::fs::create_dir_all(".tmp").unwrap();
    let dir = tempfile::tempdir_in(".tmp").unwrap();
    let path =
        std::path::absolute(dir.path().join(format!("{} 音楽.mp3", "long".repeat(90)))).unwrap();
    let source = Library {
        songs: vec![Song {
            id: 7,
            title: "A & B".into(),
            path: path.clone(),
            file_type: "mp3".into(),
            date_modified: chrono::DateTime::from_timestamp(1700000000, 0),
            cue: Some(0.0),
            cues: vec![HotCue {
                name: "A".into(),
                offset: 1.25,
                slot: 1,
                color: [0; 3],
            }],
            grid: vec![Marker {
                start: 0.5,
                bpm: 120.0,
                beat: 0,
            }],
            key: Some(1),
            rating: 80,
            ..Default::default()
        }],
        playlists: vec![Playlist {
            id: 1,
            name: "Tracks".into(),
            songs: vec![7],
            children: vec![],
        }],
        ..Default::default()
    };
    let document = Document::from_library(&source, &ExportOptions::default()).unwrap();
    let xml = document.to_xml().unwrap();
    assert!(xml.contains("DateModified=\"2023-11-14\""));
    assert!(!xml.contains("DateModidied"));
    assert!(xml.len() > 255);
    let mut document = Document::from_xml(&xml).unwrap();
    let imported = document.to_library().unwrap();
    let song = &imported.songs[0];
    assert_eq!(song.path, path);
    assert_eq!(song.cue, Some(0.0));
    assert_eq!(song.cues, source.songs[0].cues);
    assert_eq!(song.grid, source.songs[0].grid);
    assert_eq!(song.key, Some(1));
    assert_eq!(song.rating, 80);
    assert!(!song.corrupt);
    assert!(
        document
            .track_by_location(&document.collection.tracks[0].location)
            .is_some()
    );
    assert!(document.node_by_name("Tracks").is_some());
    document.remove_track(7);
    assert!(document.collection.tracks.is_empty());
    assert!(document.playlists.node.children[0].tracks.is_empty());
    let filename = dir.path().join("collection.xml");
    document.save(&filename).unwrap();
    assert!(
        Document::open(&filename)
            .unwrap()
            .collection
            .tracks
            .is_empty()
    );
}

#[test]
fn location_key_playlists_resolve_and_unknown_keys_remain_unknown() {
    let xml = r#"<DJ_PLAYLISTS Version="1.0.0"><COLLECTION Entries="1"><TRACK TrackID="10" Name="Unanalyzed" Location="file://localhost/Music/a.mp3" Tonality="unknown"/></COLLECTION><PLAYLISTS><NODE Type="0" Name="ROOT"><NODE Type="1" Name="Locations" KeyType="1"><TRACK Key="file://localhost/Music/a.mp3"/></NODE></NODE></PLAYLISTS></DJ_PLAYLISTS>"#;
    let mut document = Document::from_xml(xml).unwrap();
    let library = document.to_library().unwrap();
    assert_eq!(library.playlists[0].songs, [10]);
    assert_eq!(library.songs[0].key, None);
    assert!(!library.songs[0].corrupt);
    document.playlists.node.children[0].tracks[0].key = "missing".into();
    assert!(document.to_library().is_err());
    for invalid in [
        "<WRONG/>",
        "<DJ_PLAYLISTS><COLLECTION>",
        "<DJ_PLAYLISTS/><DJ_PLAYLISTS/>",
    ] {
        assert!(Document::from_xml(invalid).is_err(), "{invalid}");
    }
    let mut document = Document::new();
    document.playlists.node.children.push(Node {
        name: "Bad".into(),
        node_type: 1,
        tracks: vec![NodeTrack { key: "999".into() }],
        ..Default::default()
    });
    assert!(document.to_library().is_err());
}

#[test]
fn oversized_cue_identity_is_a_validation_error_without_integer_overflow() {
    let mut document = Document::new();
    document.add_track(djtools::rbxml::Track {
        id: 1,
        location: "file:///Music/a.mp3".into(),
        position_marks: vec![djtools::rbxml::PositionMark {
            num: i32::MAX,
            ..Default::default()
        }],
        ..Default::default()
    });
    assert!(document.to_library().is_err());
}

#[test]
fn unsupported_time_signature_is_not_silently_rewritten_as_four_four() {
    let mut document = Document::new();
    document.collection.tracks.push(djtools::rbxml::Track {
        id: 1,
        location: "file://localhost/music/a.mp3".into(),
        tempos: vec![djtools::rbxml::Tempo {
            start: 0.0,
            bpm: 120.0,
            meter: "3/4".into(),
            beat: 1,
        }],
        ..Default::default()
    });
    assert!(document.to_library().is_err());
    document.collection.tracks[0].tempos[0].meter = "4/4".into();
    assert_eq!(document.to_library().unwrap().songs[0].grid[0].beat, 0);
}

#[test]
fn complete_xml_conversion_handles_loops_folders_colors_ratings_and_native_document_edits() {
    use djtools::{
        Loop,
        rbxml::{PositionMark, Track},
    };
    std::fs::create_dir_all(".tmp").unwrap();
    let dir = tempfile::tempdir_in(".tmp").unwrap();
    let path = dir.path().join("collection.xml");
    let library = Library {
        songs: vec![
            Song {
                id: 2,
                path: "Music/a.flac".into(),
                title: "Track".into(),
                bpm_analyzed: Some(123.5),
                color: Some([17, 34, 51]),
                rating: 13,
                loops: vec![Loop {
                    name: "L".into(),
                    start: 1.0,
                    end: 2.0,
                    slot: 4,
                    ..Default::default()
                }],
                ..Default::default()
            },
            Song {
                id: 1,
                path: "Music/b.flac".into(),
                date_added: chrono::DateTime::from_timestamp(1700000000, 0),
                ..Default::default()
            },
        ],
        playlists: vec![Playlist {
            id: 1,
            name: "Folder".into(),
            songs: vec![2],
            children: vec![Playlist {
                id: 2,
                name: "Child".into(),
                songs: vec![1],
                children: vec![],
            }],
        }],
        ..Default::default()
    };
    djtools::rbxml::export(&library, &path, &ExportOptions { use_utc: false }).unwrap();
    let imported = djtools::rbxml::import(&path).unwrap();
    assert_eq!(imported.songs[1].loops[0].name, "L");
    assert_eq!(imported.songs[1].color, Some([17, 34, 51]));
    assert_eq!(imported.songs[1].rating, 13);
    assert_eq!(imported.songs[1].bpm, Some(123.5));
    assert_eq!(imported.playlists[0].children[0].songs, [2]);
    assert_eq!(imported.playlists[0].children[1].songs, [1]);
    let mut document = Document::open(&path).unwrap();
    let mut track = document.track_by_id(2).unwrap().clone();
    track.name = "Changed".into();
    track.position_marks.push(PositionMark {
        kind: 99,
        num: -2,
        ..Default::default()
    });
    document.add_track(track);
    assert_eq!(document.collection.tracks.len(), 2);
    assert_eq!(document.track_by_id(2).unwrap().name, "Changed");
    assert!(document.track_by_id(999).is_none());
    assert!(document.node_by_name("absent").is_none());
    document.playlists.node.children.push(Node {
        name: "By location".into(),
        node_type: 1,
        key_type: 1,
        tracks: vec![NodeTrack {
            key: document.track_by_id(2).unwrap().location.clone(),
        }],
        ..Default::default()
    });
    document.remove_track(2);
    assert!(
        document
            .playlists
            .node
            .children
            .last()
            .unwrap()
            .tracks
            .is_empty()
    );
    for (rating, expected) in [
        (1, 20),
        (2, 40),
        (3, 60),
        (4, 80),
        (5, 100),
        (51, 20),
        (102, 40),
        (153, 60),
        (204, 80),
        (255, 100),
    ] {
        let mut doc = Document::new();
        doc.add_track(Track {
            id: 1,
            location: "file:///Music/a.mp3".into(),
            rating,
            ..Default::default()
        });
        assert_eq!(doc.to_library().unwrap().songs[0].rating, expected);
    }
    for (key, expected) in [("C", 0), ("Am", 1), ("9B", 2), ("Dm", 23)] {
        let mut doc = Document::new();
        doc.add_track(Track {
            id: 1,
            location: "file:///Music/a.mp3".into(),
            tonality: key.into(),
            ..Default::default()
        });
        assert_eq!(doc.to_library().unwrap().songs[0].key, Some(expected));
    }
}

#[test]
fn invalid_xml_values_are_errors_before_an_existing_document_is_changed() {
    use djtools::rbxml::{PositionMark, Tempo, Track};
    let base = Track {
        id: 1,
        location: "file:///Music/a.mp3".into(),
        ..Default::default()
    };
    let invalid = vec![
        Track {
            location: "not a URL".into(),
            ..base.clone()
        },
        Track {
            location: "https://example.com/music".into(),
            ..base.clone()
        },
        Track {
            rating: 300,
            ..base.clone()
        },
        Track {
            color: "bad color".into(),
            ..base.clone()
        },
        Track {
            color: "0x1000000".into(),
            ..base.clone()
        },
        Track {
            date_added: "not a date".into(),
            ..base.clone()
        },
        Track {
            tempos: vec![Tempo {
                bpm: 120.0,
                beat: 5,
                ..Default::default()
            }],
            ..base.clone()
        },
        Track {
            position_marks: vec![PositionMark {
                num: 0,
                kind: 4,
                start: 2.0,
                end: 1.0,
                ..Default::default()
            }],
            ..base.clone()
        },
    ];
    for track in invalid {
        let mut doc = Document::new();
        doc.add_track(track);
        assert!(doc.to_library().is_err());
    }
    let mut doc = Document::new();
    doc.collection.tracks = vec![
        base.clone(),
        Track {
            id: 2,
            ..base.clone()
        },
    ];
    assert!(doc.to_library().is_err());
    for node in [
        Node {
            node_type: 2,
            ..Default::default()
        },
        Node {
            key_type: 2,
            ..Default::default()
        },
        Node {
            node_type: 1,
            tracks: vec![NodeTrack {
                key: "non-numeric".into(),
            }],
            ..Default::default()
        },
    ] {
        let mut doc = Document::new();
        doc.playlists.node.children = vec![node];
        assert!(doc.to_library().is_err());
    }
    let mut doc = Document::new();
    doc.playlists.node.node_type = 1;
    assert!(doc.to_library().is_err());
}

use djtools::{Library, Playlist, Song};

#[test]
fn rgb_text_conversion_is_exact_and_malformed_saved_memberships_are_rejected() {
    let rgb = djtools::parse_rgb("#Ab001f").unwrap();
    assert_eq!(rgb, [171, 0, 31]);
    assert_eq!(djtools::format_rgb(rgb), "#AB001F");
    for invalid in [
        "#12345",
        "#1234567",
        "123456",
        "#GG0011",
        "#000000garbage",
        "#音楽",
    ] {
        assert!(djtools::parse_rgb(invalid).is_err());
    }
    std::fs::create_dir_all(".tmp").unwrap();
    let dir = tempfile::tempdir_in(".tmp").unwrap();
    let path = dir.path().join("broken.json");
    let broken = Library {
        playlists: vec![Playlist {
            id: 1,
            name: "Dangling".into(),
            songs: vec![99],
            children: vec![],
        }],
        ..Default::default()
    };
    std::fs::write(&path, serde_json::to_vec(&broken).unwrap()).unwrap();
    assert!(Library::load(path).is_err());
}

#[test]
fn nested_memberships_sort_search_and_corrupt_removal_stay_consistent() {
    let mut library = Library {
        songs: vec![
            Song {
                id: 2,
                title: "UPPER".into(),
                ..Song::default()
            },
            Song {
                id: 1,
                title: "First".into(),
                corrupt: true,
                ..Song::default()
            },
        ],
        playlists: vec![Playlist {
            id: 9,
            name: "Root".into(),
            songs: vec![1, 2],
            children: vec![Playlist {
                id: 10,
                name: "Child".into(),
                songs: vec![1, 1, 2],
                ..Playlist::default()
            }],
        }],
        ..Default::default()
    };
    assert_eq!(library.song_by_title("upper").unwrap().id, 2);
    assert_eq!(library.playlist_by_id(10).unwrap().name, "Child");
    assert_eq!(library.playlist_by_name("Child").unwrap().id, 10);
    library.sort();
    assert_eq!(library.songs[0].id, 1);
    assert_eq!(library.remove_corrupt(), 1);
    assert_eq!(library.playlists[0].songs, vec![2]);
    assert_eq!(library.playlists[0].children[0].songs, vec![2]);
    assert!(library.song_by_id(1).is_none());
    library.remove_song(2);
    assert!(library.songs.is_empty());
    assert!(library.playlists[0].children[0].songs.is_empty());
}

#[test]
fn native_json_round_trip_retains_nullable_metadata_and_never_overwrites_on_invalid_input() {
    std::fs::create_dir_all(".tmp").unwrap();
    let dir = tempfile::tempdir_in(".tmp").unwrap();
    let path = dir.path().join("library.json");
    let library = Library {
        songs: vec![Song {
            id: 1,
            path: "音楽/track.flac".into(),
            ..Song::default()
        }],
        ..Library::default()
    };
    library.save(&path).unwrap();
    assert_eq!(Library::load(&path).unwrap(), library);
    assert!(
        library
            .song_by_path(std::path::Path::new("音楽/track.flac"))
            .is_some()
    );
    assert!(library.song_by_title("missing").is_none());
    assert!(library.playlist_by_id(99).is_none());
    assert!(library.playlist_by_name("missing").is_none());
    std::fs::write(&path, b"not json").unwrap();
    assert!(Library::load(&path).is_err());
}
#[test]
fn nonfinite_metadata_and_broken_memberships_cannot_overwrite_a_saved_library() {
    std::fs::create_dir_all(".tmp").unwrap();
    let dir = tempfile::tempdir_in(".tmp").unwrap();
    let path = dir.path().join("library.json");
    let mut library = djtools::Library::default();
    library.songs.push(djtools::Song {
        id: 1,
        length: 120.0,
        ..Default::default()
    });
    library.save(&path).unwrap();
    let original = std::fs::read(&path).unwrap();
    library.songs[0].length = f64::NAN;
    assert!(library.save(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    library.songs[0].length = 120.0;
    library.playlists.push(djtools::Playlist {
        id: 0,
        name: "List".into(),
        songs: vec![999],
        children: vec![],
    });
    assert!(library.validate().is_err());
}

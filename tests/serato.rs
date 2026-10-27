use base64::Engine;
use djtools::serato::{
    Crate, Markers, decode_crate, encode_crate, extract_crates, import, parse_markers_,
    parse_markers2,
};
use tokio_util::sync::CancellationToken;

fn chunk(kind: &[u8], body: &[u8]) -> Vec<u8> {
    let mut bytes = kind.to_vec();
    bytes.extend((body.len() as u32).to_be_bytes());
    bytes.extend(body);
    bytes
}
fn markers2() -> Vec<u8> {
    let mut raw = vec![1, 1];
    let mut cue = vec![0, 7];
    cue.extend(1234_u32.to_be_bytes());
    cue.extend([0, 12, 34, 56, 0, 0]);
    cue.extend("音楽".as_bytes());
    cue.push(0);
    raw.extend(chunk(b"CUE\0", &cue));
    let mut value = vec![0, 2];
    value.extend(2000_u32.to_be_bytes());
    value.extend(4000_u32.to_be_bytes());
    value.extend([255; 4]);
    value.extend([0, 40, 50, 60, 0, 1]);
    value.extend(b"Loop\0");
    raw.extend(chunk(b"LOOP\0", &value));
    raw.extend(chunk(b"COLOR\0", &[0, 1, 2, 3]));
    raw.extend(chunk(b"BPMLOCK\0", &[1]));
    raw.extend(chunk(b"FLIP\0", &[9, 8, 7]));
    raw.push(0);
    let mut data = vec![1, 1];
    data.extend(
        base64::engine::general_purpose::STANDARD
            .encode(raw)
            .bytes(),
    );
    data.resize(470, 0);
    data
}
fn packed(value: u32) -> [u8; 4] {
    let [_, a, b, c] = value.to_be_bytes();
    [
        a >> 5,
        ((a << 2) | (b >> 6)) & 127,
        ((b << 1) | (c >> 7)) & 127,
        c & 127,
    ]
}
fn legacy() -> Vec<u8> {
    let mut data = vec![2, 5];
    data.extend(14_u32.to_be_bytes());
    for index in 0..14 {
        let mut entry = vec![127; 22];
        entry[20] = if index < 5 { 1 } else { 3 };
        entry[21] = 0;
        if index == 0 {
            entry[0] = 0;
            entry[1..5].copy_from_slice(&packed(3500));
            entry[16..20].copy_from_slice(&packed(0xcc1122));
        }
        if index == 5 {
            entry[0] = 0;
            entry[1..5].copy_from_slice(&packed(1000));
            entry[5] = 0;
            entry[6..10].copy_from_slice(&packed(2500));
            entry[16..20].copy_from_slice(&packed(0x102030));
            entry[21] = 1;
        }
        data.extend(entry);
    }
    data.extend(packed(0xabcdef));
    data
}

#[test]
fn real_marker_layouts_decode_positions_names_colors_locks_and_unknown_entries() {
    let mut parsed = parse_markers2(&markers2()).unwrap();
    assert_eq!(parsed.cues[0].slot, 8);
    assert_eq!(parsed.cues[0].offset, 1.234);
    assert_eq!(parsed.cues[0].name, "音楽");
    assert_eq!(parsed.cues[0].color, [12, 34, 56]);
    assert_eq!(parsed.loops[0].slot, 3);
    assert_eq!(parsed.loops[0].end, 4.0);
    assert!(parsed.loops[0].locked);
    assert_eq!(parsed.color, Some([1, 2, 3]));
    assert_eq!(parsed.beat_grid_locked, Some(true));
    assert_eq!(parsed.unknown[0].kind, "FLIP");
    let old = parse_markers_(&legacy()).unwrap();
    assert_eq!(old.cues[0].offset, 3.5);
    assert_eq!(old.cues[0].color, [0xcc, 0x11, 0x22]);
    assert_eq!(old.loops[0].start, 1.0);
    assert!(old.loops[0].locked);
    assert_eq!(old.color, Some([0xab, 0xcd, 0xef]));
    parsed.merge_legacy(old);
    assert_eq!(parsed.cues.len(), 2);
    assert_eq!(parsed.cues[0].slot, 1);
    assert_eq!(parsed.cues[1].slot, 8);
    assert_eq!(parsed.loops.len(), 1);
    assert_eq!(parsed.loops[0].slot, 1);
    for invalid in [
        vec![],
        vec![1, 1, b'!'],
        vec![1, 1, b'A'],
        vec![2, 5, 0, 0, 0, 14],
        {
            let mut v = legacy();
            v[6 + 20] = 2;
            v
        },
    ] {
        assert!(
            if invalid.starts_with(&[2, 5]) {
                parse_markers_(&invalid)
            } else {
                parse_markers2(&invalid)
            }
            .is_err()
        );
    }
    assert_eq!(Markers::default().cues.len(), 0);
}

#[test]
fn crates_unicode_nested_chunks_priority_dedup_and_missing_audio_warnings() {
    std::fs::create_dir_all(".tmp").unwrap();
    let dir = tempfile::tempdir_in(".tmp").unwrap();
    let root = dir.path().join("_Serato_/Subcrates");
    let value = Crate {
        filename: "Alpha".into(),
        version: "2.0".into(),
        paths: vec!["/Music/音楽😀.mp3".into(), "/missing/two.flac".into()],
    };
    value.write(root.join("Alpha.crate")).unwrap();
    Crate {
        filename: "Beta".into(),
        paths: vec![value.paths[0].clone()],
        ..Default::default()
    }
    .write(root.join("Beta.crate"))
    .unwrap();
    let read = Crate::read(root.join("Alpha.crate")).unwrap();
    assert_eq!(read, value);
    let decoded = decode_crate(&encode_crate(&value).unwrap()).unwrap();
    assert_eq!(decoded.paths, value.paths);
    assert_eq!(extract_crates(dir.path()).unwrap().len(), 2);
    let report = import(dir.path(), &CancellationToken::new()).unwrap();
    assert_eq!(report.library.songs.len(), 2);
    assert_eq!(report.library.playlists[1].songs, [1]);
    assert_eq!(report.library.songs[0].title, "音楽😀");
    assert_eq!(report.warnings.len(), 2);
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(import(dir.path(), &cancelled).is_err());
    assert!(extract_crates(dir.path().join("absent")).is_err());
    let unknown = chunk(b"ufoo", &[1, 2]);
    assert!(decode_crate(&unknown).is_ok());
    assert!(decode_crate(&chunk(b"xfoo", &[])).is_err());
    for bad in [vec![1], chunk(b"ptrk", &[1]), chunk(b"ptrk", &[0xd8, 0]), {
        let mut v = chunk(b"ptrk", &[]);
        v[7] = 7;
        v
    }] {
        assert!(decode_crate(&bad).is_err());
    }
}

#[test]
fn native_audio_tags_and_utf16_geob_frames_are_extracted_without_fake_marker_parsing() {
    use lofty::{
        TextEncoding,
        config::{ParseOptions, WriteOptions},
        file::AudioFile,
        id3::v2::{BinaryFrame, Frame, FrameId, GeneralEncapsulatedObject},
        mpeg::MpegFile,
        tag::Accessor,
    };
    std::fs::create_dir_all(".tmp").unwrap();
    let dir = tempfile::tempdir_in(".tmp").unwrap();
    let path = dir.path().join("fallback.mp3");
    let generated = std::process::Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=0.2",
            "-y",
        ])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let mut file = MpegFile::read_from(
        &mut std::fs::File::open(&path).unwrap(),
        ParseOptions::new(),
    )
    .unwrap();
    let tag = file.id3v2_mut().unwrap();
    tag.set_title("Actual title".into());
    tag.set_artist("Artist".into());
    tag.set_album("Album".into());
    tag.set_genre("House".into());
    tag.set_track(7);
    tag.set_date("2024-03-05".parse().unwrap());
    tag.insert(Frame::Text(lofty::id3::v2::TextInformationFrame::new(
        FrameId::Valid("TBPM".into()),
        TextEncoding::UTF8,
        "123",
    )));
    tag.insert(Frame::Text(lofty::id3::v2::TextInformationFrame::new(
        FrameId::Valid("TKEY".into()),
        TextEncoding::UTF8,
        "Am",
    )));
    for (name, data) in [
        ("Serato Markers2", markers2()),
        ("Serato Markers_", legacy()),
        ("Unknown 音楽", vec![1, 2, 3]),
    ] {
        let object = GeneralEncapsulatedObject::new(
            TextEncoding::UTF16,
            Some("application/octet-stream".into()),
            None,
            Some(name.into()),
            data,
        );
        tag.insert(Frame::Binary(BinaryFrame::new(
            FrameId::Valid("GEOB".into()),
            object.as_bytes(WriteOptions::new()).unwrap(),
        )));
    }
    file.save_to_path(&path, WriteOptions::new()).unwrap();
    let extracted = djtools::serato::extract_song(&path).unwrap();
    assert_eq!(extracted.song.title, "Actual title");
    assert_eq!(extracted.song.bpm, Some(123.0));
    assert_eq!(extracted.song.key, Some(1));
    assert_eq!(extracted.song.year, Some(2024));
    assert_eq!(extracted.song.track_number, 7);
    assert_eq!(extracted.objects.len(), 3);
    assert!(
        extracted
            .objects
            .iter()
            .any(|object| object.descriptor == "Unknown 音楽" && object.data == [1, 2, 3])
    );
    assert_eq!(extracted.song.cues[0].offset, 3.5);
    assert_eq!(extracted.song.cues[1].name, "音楽");
    assert!(extracted.song.length > 0.0);
    assert_eq!(extracted.song.sample_rate, Some(44100.0));
    let crate_path = dir.path().join("Tracks.crate");
    Crate {
        paths: vec![path.clone()],
        ..Default::default()
    }
    .write(&crate_path)
    .unwrap();
    let report = import(dir.path(), &CancellationToken::new()).unwrap();
    assert!(report.warnings.is_empty());
    assert_eq!(report.library.songs[0].artist, "Artist");
}

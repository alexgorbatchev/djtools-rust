use crate::{Library, Playlist, Result, Song};
use std::{collections::BTreeSet, io::Write, path::Path};

impl Library {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let library: Self =
            serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(path)?))?;
        library.validate()?;
        Ok(library)
    }
    /// Serialize before publication, so errors leave an existing library intact.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        self.validate()?;
        let path = path.as_ref();
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut temporary, self)?;
        temporary.write_all(b"\n")?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|error| error.error)?;
        Ok(())
    }
    /// Check portable identities, memberships, positions, and finite numeric metadata.
    pub fn validate(&self) -> Result<()> {
        let mut songs = BTreeSet::new();
        for song in &self.songs {
            if song.id < 0 || !songs.insert(song.id) {
                return Err(crate::Error::Invalid(
                    "negative or duplicate song identity".into(),
                ));
            }
            if !song.length.is_finite()
                || song.length < 0.0
                || song.rating > 100
                || song.key.is_some_and(|key| key > 23)
            {
                return Err(crate::Error::Invalid(
                    "invalid duration, rating, or musical key".into(),
                ));
            }
            for value in [
                song.bpm,
                song.bpm_analyzed,
                song.sample_rate,
                song.cue,
                song.engine
                    .as_ref()
                    .and_then(|record| record.record.track.bpm_analyzed),
            ]
            .into_iter()
            .flatten()
            {
                if !value.is_finite() {
                    return Err(crate::Error::Invalid("nonfinite song metadata".into()));
                }
            }
            if song.sample_rate.is_some_and(|rate| rate <= 0.0)
                || song.cue.is_some_and(|cue| cue < 0.0)
            {
                return Err(crate::Error::Invalid(
                    "invalid sample rate or main cue".into(),
                ));
            }
            for marker in &song.grid {
                if !marker.start.is_finite()
                    || !marker.bpm.is_finite()
                    || marker.bpm <= 0.0
                    || marker.beat > 3
                {
                    return Err(crate::Error::Invalid("invalid beat grid".into()));
                }
            }
            let mut cues = BTreeSet::new();
            let mut loops = BTreeSet::new();
            for cue in &song.cues {
                if !cue.offset.is_finite()
                    || cue.offset < 0.0
                    || cue.slot == 0
                    || !cues.insert(cue.slot)
                {
                    return Err(crate::Error::Invalid("invalid or duplicate hot cue".into()));
                }
            }
            for value in &song.loops {
                if !value.start.is_finite()
                    || !value.end.is_finite()
                    || value.start < 0.0
                    || value.end < value.start
                    || value.slot == 0
                    || !loops.insert(value.slot)
                {
                    return Err(crate::Error::Invalid("invalid or duplicate loop".into()));
                }
            }
        }
        validate_lists(&self.playlists, &songs, &mut BTreeSet::new(), 0)
    }
    pub fn song_by_id(&self, id: i64) -> Option<&Song> {
        self.songs.iter().find(|song| song.id == id)
    }
    pub fn song_by_path(&self, path: &Path) -> Option<&Song> {
        self.songs.iter().find(|song| song.path == path)
    }
    pub fn song_by_title(&self, title: &str) -> Option<&Song> {
        self.songs
            .iter()
            .find(|song| song.title.to_lowercase() == title.to_lowercase())
    }
    pub fn playlist_by_id(&self, id: i64) -> Option<&Playlist> {
        lookup(&self.playlists, &|playlist| playlist.id == id)
    }
    pub fn playlist_by_name(&self, name: &str) -> Option<&Playlist> {
        lookup(&self.playlists, &|playlist| playlist.name == name)
    }
    pub fn sort(&mut self) {
        self.songs.sort_by_key(|song| song.id);
        for song in &mut self.songs {
            song.cues.sort_by_key(|cue| cue.slot);
            song.loops.sort_by_key(|value| value.slot);
        }
    }
    pub fn add_song(&mut self, song: Song) {
        self.songs.push(song);
    }
    pub fn remove_song(&mut self, id: i64) {
        self.songs.retain(|song| song.id != id);
        remove_members(&mut self.playlists, &BTreeSet::from([id]));
    }
    pub fn remove_corrupt(&mut self) -> usize {
        let removed: BTreeSet<_> = self
            .songs
            .iter()
            .filter(|song| song.corrupt)
            .map(|song| song.id)
            .collect();
        let count = self.songs.len();
        self.songs.retain(|song| !removed.contains(&song.id));
        remove_members(&mut self.playlists, &removed);
        count - self.songs.len()
    }
}

fn validate_lists(
    lists: &[Playlist],
    songs: &BTreeSet<i64>,
    identities: &mut BTreeSet<i64>,
    depth: usize,
) -> Result<()> {
    if depth > 128 {
        return Err(crate::Error::Invalid(
            "playlist hierarchy exceeds 128 levels".into(),
        ));
    }
    for list in lists {
        if list.id < 0 || (list.id != 0 && !identities.insert(list.id)) {
            return Err(crate::Error::Invalid(
                "negative or duplicate playlist identity".into(),
            ));
        }
        if list.songs.iter().any(|id| !songs.contains(id)) {
            return Err(crate::Error::Invalid(
                "playlist references an absent song".into(),
            ));
        }
        validate_lists(&list.children, songs, identities, depth + 1)?;
    }
    Ok(())
}

fn lookup<'a>(lists: &'a [Playlist], matches: &impl Fn(&Playlist) -> bool) -> Option<&'a Playlist> {
    for playlist in lists {
        if matches(playlist) {
            return Some(playlist);
        }
        if let Some(found) = lookup(&playlist.children, matches) {
            return Some(found);
        }
    }
    None
}
fn remove_members(lists: &mut [Playlist], removed: &BTreeSet<i64>) {
    for playlist in lists {
        playlist.songs.retain(|id| !removed.contains(id));
        remove_members(&mut playlist.children, removed);
    }
}

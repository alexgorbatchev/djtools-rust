//! Rekordbox's documented XML interchange format.
mod convert;
mod model;
use crate::{Error, Library, Result};
pub(crate) use convert::musical_key;
pub use model::*;
use serde::{Deserialize, Serialize};
use std::{io::Write, path::Path};

#[derive(Clone, Copy, Debug)]
pub struct ExportOptions {
    pub use_utc: bool,
}
impl Default for ExportOptions {
    fn default() -> Self {
        Self { use_utc: true }
    }
}

#[derive(Deserialize)]
enum Root {
    #[serde(rename = "DJ_PLAYLISTS")]
    Document(Document),
}

impl Document {
    pub fn new() -> Self {
        Self {
            version: "1.0.0".into(),
            product: Product {
                name: "djtools".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                company: "djtools".into(),
            },
            collection: Collection::default(),
            playlists: Playlists {
                node: Node {
                    name: "ROOT".into(),
                    ..Node::default()
                },
            },
        }
    }
    pub fn from_xml(xml: &str) -> Result<Self> {
        let mut de = quick_xml::de::Deserializer::from_str(xml);
        let Root::Document(document) = Root::deserialize(&mut de)?;
        if !de.is_empty() {
            return Err(Error::Invalid("trailing XML document content".into()));
        }
        Ok(document)
    }
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_xml(&std::fs::read_to_string(path)?)
    }
    pub fn to_xml(&self) -> Result<String> {
        self.to_library()?;
        let mut copy = self.clone();
        copy.collection.entries = copy.collection.tracks.len();
        counts(&mut copy.playlists.node);
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        let mut serializer = quick_xml::se::Serializer::new(&mut xml);
        serializer.indent(' ', 2);
        copy.serialize(serializer)?;
        Ok(xml)
    }
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let xml = self.to_xml()?;
        let path = path.as_ref();
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(xml.as_bytes())?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|error| error.error)?;
        Ok(())
    }
    pub fn track_by_id(&self, id: i64) -> Option<&Track> {
        self.collection.tracks.iter().find(|track| track.id == id)
    }
    pub fn track_by_location(&self, location: &str) -> Option<&Track> {
        self.collection
            .tracks
            .iter()
            .find(|track| track.location == location)
    }
    pub fn add_track(&mut self, track: Track) {
        if let Some(existing) = self
            .collection
            .tracks
            .iter_mut()
            .find(|existing| existing.id == track.id)
        {
            *existing = track;
        } else {
            self.collection.tracks.push(track);
        }
        self.collection.entries = self.collection.tracks.len();
    }
    pub fn remove_track(&mut self, id: i64) {
        let locations: Vec<_> = self
            .collection
            .tracks
            .iter()
            .filter(|track| track.id == id)
            .map(|track| track.location.clone())
            .collect();
        self.collection.tracks.retain(|track| track.id != id);
        self.collection.entries = self.collection.tracks.len();
        remove(&mut self.playlists.node, id, &locations);
    }
    pub fn node_by_name(&self, name: &str) -> Option<&Node> {
        node(&self.playlists.node, name)
    }
    pub fn sort(&mut self) {
        self.collection.tracks.sort_by_key(|track| track.id);
        for track in &mut self.collection.tracks {
            track
                .position_marks
                .sort_by_key(|mark| (mark.kind, mark.num));
        }
    }
    pub fn from_library(library: &Library, options: &ExportOptions) -> Result<Self> {
        convert::from_library(library, options)
    }
    pub fn to_library(&self) -> Result<Library> {
        convert::to_library(self)
    }
}

fn counts(node: &mut Node) {
    node.count = node.children.len();
    node.entries = node.tracks.len();
    for child in &mut node.children {
        counts(child);
    }
}
fn node<'a>(root: &'a Node, name: &str) -> Option<&'a Node> {
    if root.name == name {
        return Some(root);
    }
    root.children.iter().find_map(|child| node(child, name))
}
fn remove(node: &mut Node, id: i64, locations: &[String]) {
    node.tracks.retain(|track| {
        if node.key_type == 1 {
            !locations.contains(&track.key)
        } else {
            track.key.parse::<i64>().ok() != Some(id)
        }
    });
    node.entries = node.tracks.len();
    for child in &mut node.children {
        remove(child, id, locations);
    }
}
pub fn import(path: impl AsRef<Path>) -> Result<Library> {
    Document::open(path)?.to_library()
}
pub fn export(library: &Library, path: impl AsRef<Path>, options: &ExportOptions) -> Result<()> {
    let mut doc = Document::from_library(library, options)?;
    doc.sort();
    doc.save(path)
}

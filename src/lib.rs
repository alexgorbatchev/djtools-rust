//! Native collection interchange. No CLI or network operations are performed.
//!
//! ```
//! use djtools::{Library, Song, rbxml};
//!
//! # fn main() -> djtools::Result<()> {
//! let library = Library {
//!     songs: vec![Song {
//!         id: 1,
//!         title: "Example track".into(),
//!         path: std::env::current_dir()?.join("Music/example.mp3"),
//!         length: 180.0,
//!         ..Song::default()
//!     }],
//!     ..Library::default()
//! };
//! let document = rbxml::Document::from_library(&library, &rbxml::ExportOptions::default())?;
//! let restored = rbxml::Document::from_xml(&document.to_xml()?)?.to_library()?;
//! assert_eq!(restored.songs[0].title, "Example track");
//! # Ok(())
//! # }
//! ```
pub mod engine;
mod library;
mod model;
pub mod rbxml;
pub mod serato;
pub use model::*;

pub fn format_rgb([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}
pub fn parse_rgb(value: &str) -> Result<[u8; 3]> {
    let hex = value
        .strip_prefix('#')
        .filter(|hex| hex.len() == 6 && hex.is_ascii())
        .ok_or_else(|| Error::Invalid("RGB must be #RRGGBB".into()))?;
    let rgb =
        u32::from_str_radix(hex, 16).map_err(|_| Error::Invalid("RGB must be #RRGGBB".into()))?;
    Ok([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8])
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("collection operation cancelled")]
    Cancelled,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    XmlRead(#[from] quick_xml::DeError),
    #[error(transparent)]
    XmlWrite(#[from] quick_xml::SeError),
    #[error(transparent)]
    Engine(#[from] engine_library::Error),
    #[error(
        "database publication failed: {cause}; rollback failures: {rollback:?}; recovery files: {recovery:?}"
    )]
    Publication {
        cause: std::io::Error,
        rollback: Vec<String>,
        recovery: Option<std::path::PathBuf>,
    },
}
pub type Result<T> = std::result::Result<T, Error>;

fn check_cancel(token: &tokio_util::sync::CancellationToken) -> Result<()> {
    if token.is_cancelled() {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

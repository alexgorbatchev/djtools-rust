use crate::{Error, Result};
use std::path::{Path, PathBuf};

#[cfg(test)]
#[path = "../../tests/unit/publication.rs"]
mod tests;
pub(super) fn publish(staging: &Path, destination: &Path) -> Result<()> {
    let backup = tempfile::Builder::new()
        .prefix(".djtools-backup-")
        .tempdir_in(destination)?;
    let mut saved = Vec::<PathBuf>::new();
    let mut published = Vec::<PathBuf>::new();
    let result = (|| -> std::io::Result<()> {
        for name in ["m.db", "hm.db"] {
            let target = destination.join(name);
            if target.try_exists()? {
                std::fs::rename(&target, backup.path().join(name))?;
                saved.push(target);
            }
        }
        for name in ["m.db", "hm.db"] {
            let target = destination.join(name);
            std::fs::rename(staging.join(name), &target)?;
            published.push(target);
        }
        Ok(())
    })();
    if let Err(cause) = result {
        let mut rollback = Vec::new();
        for target in published {
            if let Err(error) = std::fs::remove_file(&target) {
                rollback.push(format!("remove {}: {error}", target.display()));
            }
        }
        for target in saved {
            if let Err(error) = std::fs::rename(
                backup
                    .path()
                    .join(target.file_name().expect("database name")),
                &target,
            ) {
                rollback.push(format!("restore {}: {error}", target.display()));
            }
        }
        let recovery = if rollback.is_empty() {
            None
        } else {
            Some(backup.keep())
        };
        return Err(Error::Publication {
            cause,
            rollback,
            recovery,
        });
    }
    Ok(())
}

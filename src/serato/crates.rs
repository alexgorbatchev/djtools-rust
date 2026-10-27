use crate::{Error, Result};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Crate {
    pub filename: String,
    pub version: String,
    pub paths: Vec<PathBuf>,
}

impl Crate {
    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let mut value = decode_crate(&std::fs::read(path)?)?;
        value.filename = path
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or_else(|| Error::Invalid("crate filename is not UTF-8".into()))?
            .into();
        Ok(value)
    }
    /// Publish a complete crate atomically, creating missing parent directories.
    pub fn write(&self, path: impl AsRef<Path>) -> Result<()> {
        let data = encode_crate(self)?;
        let path = path.as_ref();
        let parent = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(&data)?;
        file.as_file().sync_all()?;
        file.persist(path).map_err(|error| error.error)?;
        Ok(())
    }
}
pub fn decode_crate(data: &[u8]) -> Result<Crate> {
    let mut value = Crate::default();
    decode_chunks(data, &mut value, 0)?;
    Ok(value)
}
fn decode_chunks(mut data: &[u8], value: &mut Crate, depth: usize) -> Result<()> {
    if depth > 128 {
        return Err(Error::Invalid("crate nesting exceeds 128 levels".into()));
    }
    while !data.is_empty() {
        if data.len() < 8 {
            return Err(Error::Invalid("truncated crate chunk header".into()));
        }
        let key = &data[..4];
        let length = u32::from_be_bytes(data[4..8].try_into().unwrap()) as usize;
        let body = data
            .get(8..)
            .and_then(|remaining| remaining.get(..length))
            .ok_or_else(|| Error::Invalid("truncated crate chunk payload".into()))?;
        data = &data[8 + length..];
        match key {
            b"otrk" => decode_chunks(body, value, depth + 1)?,
            b"vrsn" => value.version = utf16(body)?,
            b"ptrk" => value
                .paths
                .push(Path::new(std::path::MAIN_SEPARATOR_STR).join(utf16(body)?)),
            _ if matches!(key[0], b'o' | b't' | b'p' | b'u' | b's' | b'b') => {}
            _ => return Err(Error::Invalid("unsupported crate chunk type".into())),
        }
    }
    Ok(())
}
fn utf16(data: &[u8]) -> Result<String> {
    if !data.len().is_multiple_of(2) {
        return Err(Error::Invalid("odd UTF-16 crate string length".into()));
    }
    String::from_utf16(
        &data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
            .collect::<Vec<_>>(),
    )
    .map_err(|_| Error::Invalid("invalid UTF-16 crate string".into()))
}
fn text(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_be_bytes).collect()
}
fn chunk(output: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) -> Result<()> {
    let length = u32::try_from(body.len())
        .map_err(|_| Error::Invalid("crate chunk exceeds 32-bit length".into()))?;
    output.extend(kind);
    output.extend(length.to_be_bytes());
    output.extend(body);
    Ok(())
}
pub fn encode_crate(value: &Crate) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    chunk(
        &mut bytes,
        b"vrsn",
        &text(if value.version.is_empty() {
            "1.0/8.0"
        } else {
            &value.version
        }),
    )?;
    for path in &value.paths {
        let path = path
            .to_str()
            .ok_or_else(|| Error::Invalid("crate paths must be UTF-8".into()))?;
        let path = path.strip_prefix(std::path::MAIN_SEPARATOR).unwrap_or(path);
        let mut inner = Vec::new();
        chunk(&mut inner, b"ptrk", &text(path))?;
        chunk(&mut bytes, b"otrk", &inner)?;
    }
    Ok(bytes)
}
pub fn extract_crates(base: impl AsRef<Path>) -> Result<Vec<Crate>> {
    let base = base.as_ref();
    for directory in [
        base.join("_Serato_/Subcrates"),
        base.join("Subcrates"),
        base.to_owned(),
    ] {
        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        let mut files = Vec::new();
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_dir()
                && entry.path().extension().is_some_and(|ext| ext == "crate")
            {
                files.push(entry.path());
            }
        }
        if !files.is_empty() {
            files.sort();
            return files.into_iter().map(Crate::read).collect();
        }
    }
    Err(Error::Invalid(format!(
        "no .crate files found in {}",
        base.display()
    )))
}

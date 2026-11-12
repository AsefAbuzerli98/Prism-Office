use std::collections::BTreeMap;
use std::io::{Cursor, Read, Seek, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::content_types::CONTENT_TYPES_PART;
use crate::{rel_types, ContentTypes, DocumentKind, Error, PartName, Relationships, Result};

/// Defensive limits applied while reading untrusted packages.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_parts: usize,
    /// Maximum uncompressed size of a single part, in bytes.
    pub max_part_size: u64,
    /// Maximum total uncompressed size of all parts, in bytes.
    pub max_total_size: u64,
    /// Maximum uncompressed/compressed ratio for parts larger than 1 MiB.
    pub max_compression_ratio: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_parts: 50_000,
            max_part_size: 512 * 1024 * 1024,
            max_total_size: 2 * 1024 * 1024 * 1024,
            max_compression_ratio: 200,
        }
    }
}

/// An in-memory OPC package.
///
/// All parts (including `.rels` parts) are stored as raw bytes keyed by
/// normalised part name, which keeps unknown content lossless on round-trip.
/// `[Content_Types].xml` is held in parsed form and regenerated on save.
#[derive(Clone, Debug, Default)]
pub struct Package {
    parts: BTreeMap<PartName, Vec<u8>>,
    content_types: ContentTypes,
}

impl Package {
    /// An empty package with standard `rels`/`xml` defaults.
    pub fn new() -> Self {
        Self {
            parts: BTreeMap::new(),
            content_types: ContentTypes::with_standard_defaults(),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Self::read(Cursor::new(bytes), Limits::default())
    }

    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        Self::read(std::io::BufReader::new(file), Limits::default())
    }

    pub fn read<R: Read + Seek>(reader: R, limits: Limits) -> Result<Self> {
        let mut zip = ZipArchive::new(reader)?;
        if zip.len() > limits.max_parts {
            return Err(Error::LimitExceeded(format!(
                "{} entries (max {})",
                zip.len(),
                limits.max_parts
            )));
        }

        let mut parts = BTreeMap::new();
        let mut content_types = None;
        let mut total: u64 = 0;

        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            if entry.is_dir() {
                continue;
            }
            let name = entry.name()?.into_owned();
            let declared = entry.size();
            let compressed = entry.compressed_size().max(1);
            if declared > limits.max_part_size {
                return Err(Error::LimitExceeded(format!(
                    "part `{name}` is {declared} bytes"
                )));
            }
            if declared > 1024 * 1024 && declared / compressed > limits.max_compression_ratio {
                return Err(Error::LimitExceeded(format!(
                    "part `{name}` compression ratio {} exceeds {}",
                    declared / compressed,
                    limits.max_compression_ratio
                )));
            }

            // Never trust the declared size: cap the actual read as well.
            let mut data = Vec::with_capacity(declared.min(16 * 1024 * 1024) as usize);
            (&mut entry)
                .take(limits.max_part_size + 1)
                .read_to_end(&mut data)?;
            if data.len() as u64 > limits.max_part_size {
                return Err(Error::LimitExceeded(format!(
                    "part `{name}` exceeds size limit"
                )));
            }
            total += data.len() as u64;
            if total > limits.max_total_size {
                return Err(Error::LimitExceeded("total uncompressed size".into()));
            }

            if name.eq_ignore_ascii_case(CONTENT_TYPES_PART) {
                content_types = Some(ContentTypes::parse(&data)?);
            } else {
                // PartName::new rejects traversal (`..`), backslashes and empty segments.
                parts.insert(PartName::new(&name)?, data);
            }
        }

        let content_types =
            content_types.ok_or_else(|| Error::MissingPart(CONTENT_TYPES_PART.into()))?;
        Ok(Self {
            parts,
            content_types,
        })
    }

    /// Serialise to a ZIP byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut cursor = Cursor::new(Vec::new());
        self.write(&mut cursor)?;
        Ok(cursor.into_inner())
    }

    /// Serialise to any seekable writer. Output is deterministic: content
    /// types first, then parts in normalised-name order, fixed timestamps.
    pub fn write<W: Write + Seek>(&self, writer: W) -> Result<()> {
        let mut zip = ZipWriter::new(writer);
        let opts = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default());

        zip.start_file(CONTENT_TYPES_PART, opts)?;
        zip.write_all(self.content_types.to_xml().as_bytes())?;
        for (name, data) in &self.parts {
            zip.start_file(name.zip_name(), opts)?;
            zip.write_all(data)?;
        }
        zip.finish()?;
        Ok(())
    }

    /// Atomically save to `path`: write to a sibling temp file, fsync, rename.
    pub fn save(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let path = path.as_ref();
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "document".into());
        let tmp = dir.join(format!(".{file_name}.prism-tmp"));
        {
            let file = std::fs::File::create(&tmp)?;
            let mut buf = std::io::BufWriter::new(file);
            let bytes = self.to_bytes()?;
            buf.write_all(&bytes)?;
            buf.flush()?;
            buf.get_ref().sync_all()?;
        }
        if let Err(e) = std::fs::rename(&tmp, path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.into());
        }
        Ok(())
    }

    // ---- parts -----------------------------------------------------------

    pub fn content_types(&self) -> &ContentTypes {
        &self.content_types
    }

    pub fn content_types_mut(&mut self) -> &mut ContentTypes {
        &mut self.content_types
    }

    pub fn part(&self, name: &PartName) -> Option<&[u8]> {
        self.parts.get(name).map(Vec::as_slice)
    }

    pub fn part_str(&self, name: &str) -> Option<&[u8]> {
        PartName::new(name).ok().and_then(|n| self.part(&n))
    }

    pub fn has_part(&self, name: &PartName) -> bool {
        self.parts.contains_key(name)
    }

    pub fn part_names(&self) -> impl Iterator<Item = &PartName> {
        self.parts.keys()
    }

    pub fn len(&self) -> usize {
        self.parts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// Insert or replace a part. If `content_type` is given it is recorded as
    /// an override (unless it equals the extension default).
    pub fn set_part(&mut self, name: PartName, data: Vec<u8>, content_type: Option<&str>) {
        if let Some(ct) = content_type {
            let matches_default = name
                .extension()
                .and_then(|e| self.content_types.default_for(&e))
                == Some(ct);
            if !matches_default {
                self.content_types.set_override(name.clone(), ct);
            }
        }
        self.parts.insert(name, data);
    }

    pub fn remove_part(&mut self, name: &PartName) -> Option<Vec<u8>> {
        self.content_types.remove_override(name);
        self.parts.remove(name)
    }

    pub fn content_type(&self, name: &PartName) -> Option<&str> {
        self.content_types.get(name)
    }

    // ---- relationships ---------------------------------------------------

    fn rels_name(source: Option<&PartName>) -> PartName {
        match source {
            Some(p) => p.rels_part(),
            None => PartName::new("/_rels/.rels").expect("static name"),
        }
    }

    /// Relationships owned by `source` (`None` = package root). Missing rels
    /// part yields an empty set.
    pub fn relationships(&self, source: Option<&PartName>) -> Result<Relationships> {
        let name = Self::rels_name(source);
        match self.parts.get(&name) {
            Some(data) => Relationships::parse(name.as_str(), data),
            None => Ok(Relationships::default()),
        }
    }

    pub fn set_relationships(&mut self, source: Option<&PartName>, rels: &Relationships) {
        let name = Self::rels_name(source);
        self.parts.insert(name, rels.to_xml().into_bytes());
    }

    /// The main document part, located via the root `officeDocument` relationship.
    pub fn main_part(&self) -> Result<PartName> {
        let rels = self.relationships(None)?;
        let rel = rels
            .first_of_type(rel_types::OFFICE_DOCUMENT)
            .or_else(|| rels.first_of_type(rel_types::OFFICE_DOCUMENT_STRICT))
            .ok_or_else(|| Error::MissingPart("officeDocument relationship".into()))?;
        rel.resolve(None)
            .ok_or_else(|| Error::MissingPart("internal officeDocument target".into()))?
    }

    /// Detect whether this is a document, workbook or presentation.
    pub fn kind(&self) -> Result<DocumentKind> {
        let main = self.main_part()?;
        let ct = self.content_type(&main).ok_or(Error::UnknownKind)?;
        DocumentKind::from_main_content_type(ct).ok_or(Error::UnknownKind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TargetMode;

    fn sample() -> Package {
        let mut p = Package::new();
        let doc = PartName::new("/word/document.xml").unwrap();
        p.set_part(
            doc.clone(),
            b"<w:document/>".to_vec(),
            Some(DocumentKind::Document.main_content_type()),
        );
        let mut rels = Relationships::default();
        rels.add(
            rel_types::OFFICE_DOCUMENT,
            "word/document.xml",
            TargetMode::Internal,
        );
        p.set_relationships(None, &rels);
        p
    }

    #[test]
    fn roundtrip_is_lossless_and_deterministic() {
        let p = sample();
        let a = p.to_bytes().unwrap();
        let b = p.to_bytes().unwrap();
        assert_eq!(a, b, "serialisation must be deterministic");

        let q = Package::from_bytes(&a).unwrap();
        assert_eq!(q.kind().unwrap(), DocumentKind::Document);
        assert_eq!(q.part_str("word/document.xml"), Some(&b"<w:document/>"[..]));
        assert_eq!(q.to_bytes().unwrap(), a);
    }

    #[test]
    fn xml_override_not_written_when_equal_to_default() {
        let mut p = Package::new();
        let n = PartName::new("/custom.xml").unwrap();
        p.set_part(n.clone(), vec![], Some("application/xml"));
        assert_eq!(p.content_types().overrides().count(), 0);
        assert_eq!(p.content_type(&n), Some("application/xml"));
    }

    #[test]
    fn missing_content_types_is_error() {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut cursor);
            zip.start_file("a.xml", SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"<a/>").unwrap();
            zip.finish().unwrap();
        }
        assert!(matches!(
            Package::from_bytes(cursor.get_ref()),
            Err(Error::MissingPart(_))
        ));
    }

    #[test]
    fn rejects_path_traversal_entries() {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut cursor);
            zip.start_file("[Content_Types].xml", SimpleFileOptions::default())
                .unwrap();
            zip.write_all(ContentTypes::with_standard_defaults().to_xml().as_bytes())
                .unwrap();
            zip.start_file("../evil.xml", SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"<x/>").unwrap();
            zip.finish().unwrap();
        }
        assert!(matches!(
            Package::from_bytes(cursor.get_ref()),
            Err(Error::InvalidPartName(_))
        ));
    }

    #[test]
    fn enforces_part_size_limit() {
        let p = {
            let mut p = sample();
            p.set_part(PartName::new("/big.bin").unwrap(), vec![0u8; 4096], None);
            p
        };
        let bytes = p.to_bytes().unwrap();
        let limits = Limits {
            max_part_size: 1024,
            ..Limits::default()
        };
        assert!(matches!(
            Package::read(Cursor::new(bytes), limits),
            Err(Error::LimitExceeded(_))
        ));
    }

    #[test]
    fn atomic_save_and_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.docx");
        sample().save(&path).unwrap();
        let q = Package::open(&path).unwrap();
        assert_eq!(q.kind().unwrap(), DocumentKind::Document);
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(
            leftovers.len(),
            1,
            "temp file must not remain: {leftovers:?}"
        );
    }
}

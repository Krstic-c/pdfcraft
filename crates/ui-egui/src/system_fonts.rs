//! The last-resort interface fonts: faces already installed on this machine, at most one per
//! probed script.
//!
//! The embedded faces (Inter, egui's defaults, craft-fonts) come first in every family; these
//! only draw characters none of them has, such as an Arabic file name or a Chinese interface in
//! a build without craft-fonts. A single face is not reliably good for every such script (an
//! Arabic-capable face is not reliably Han-capable, and vice versa), so each probed script gets
//! its own slot rather than all of them competing for one. They are read at runtime and never
//! embedded or shipped (AGENTS.md §1.4), and `PDFCRAFT_SYSTEM_FONTS=0` turns them off (published
//! screenshots do).

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use egui::FontData;

/// Larger files are not read: a font path is still untrusted input.
const MAX_BYTES: u64 = 32 << 20;
/// Apple's Chinese collections (PingFang, STHeiti) exceed `MAX_BYTES`.
const MAX_CHINESE_BYTES: u64 = 128 << 20;
/// Faces tried in a collection (`.ttc`).
const MAX_FACES: u32 = 16;
/// Arabic letter alef: the default probe, for candidates expected to have broad coverage.
const ARABIC: char = '\u{0627}';
/// A CJK ideograph: the probe for candidates added specifically for Han coverage (e.g. a
/// Chinese interface), since an Arabic-capable face is not reliably Han-capable and vice versa.
const HAN: char = '\u{4e2d}';

/// The installed fallback faces, read once: the first candidate found for each distinct probe
/// character, best candidate first. Empty when it is turned off or no candidate fits.
pub fn fallback() -> Vec<(char, Arc<FontData>)> {
    static CACHE: OnceLock<Vec<(char, Arc<FontData>)>> = OnceLock::new();
    CACHE.get_or_init(load).clone()
}

fn load() -> Vec<(char, Arc<FontData>)> {
    if std::env::var_os("PDFCRAFT_SYSTEM_FONTS").is_some_and(|v| v == "0") {
        return Vec::new();
    }
    let mut found: Vec<(char, Arc<FontData>)> = Vec::new();
    for (path, probe, max_bytes) in candidates() {
        if found.iter().any(|&(p, _)| p == probe) {
            continue;
        }
        if let Some(data) = read(&path, probe, max_bytes) {
            found.push((probe, data));
        }
    }
    found
}

/// Well-known locations of faces with broad script coverage, best first, each paired with the
/// character it must map to be accepted and the size limit its file gets checked against.
fn candidates() -> Vec<(PathBuf, char, u64)> {
    if cfg!(windows) {
        let dir = std::env::var_os("WINDIR").or_else(|| std::env::var_os("SystemRoot")).map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        [("msyh.ttc", HAN, MAX_BYTES), ("segoeui.ttf", ARABIC, MAX_BYTES), ("tahoma.ttf", ARABIC, MAX_BYTES), ("arial.ttf", ARABIC, MAX_BYTES)]
            .iter()
            .map(|(f, probe, max_bytes)| (dir.join("Fonts").join(f), *probe, *max_bytes))
            .collect()
    } else if cfg!(target_os = "macos") {
        [
            // The pre-2025 location; still the first place to look on older macOS.
            ("/System/Library/Fonts/PingFang.ttc", HAN, MAX_CHINESE_BYTES),
            ("/System/Library/Fonts/STHeiti Light.ttc", HAN, MAX_CHINESE_BYTES),
            ("/System/Library/Fonts/STHeiti Medium.ttc", HAN, MAX_CHINESE_BYTES),
            ("/System/Library/Fonts/SFArabic.ttf", ARABIC, MAX_BYTES),
            ("/System/Library/Fonts/GeezaPro.ttc", ARABIC, MAX_BYTES),
            ("/System/Library/Fonts/Supplemental/Arial.ttf", ARABIC, MAX_BYTES),
        ]
        .iter()
        .map(|(f, probe, max_bytes)| (PathBuf::from(f), *probe, *max_bytes))
        .collect()
    } else {
        let files = [
            ("truetype/noto/NotoSansArabic-Regular.ttf", ARABIC),
            ("noto/NotoSansArabic-Regular.ttf", ARABIC),
            ("google-noto/NotoSansArabic-Regular.ttf", ARABIC),
            ("truetype/dejavu/DejaVuSans.ttf", ARABIC),
            ("TTF/DejaVuSans.ttf", ARABIC),
            ("dejavu/DejaVuSans.ttf", ARABIC),
            ("dejavu-sans-fonts/DejaVuSans.ttf", ARABIC),
        ];
        ["/usr/share/fonts", "/usr/local/share/fonts"]
            .iter()
            .flat_map(|dir| files.iter().map(move |(f, probe)| (Path::new(dir).join(f), *probe, MAX_BYTES)))
            .collect()
    }
}

fn read(path: &Path, probe: char, max_bytes: u64) -> Option<Arc<FontData>> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > max_bytes {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let index = face_with(&bytes, probe)?;
    let mut data = FontData::from_owned(bytes);
    data.index = index;
    log::info!("interface font fallback: {}", path.display());
    Some(Arc::new(data))
}

/// The first face of the file that parses and maps `c`. egui parses fonts with the same skrifa,
/// so a face accepted here is one it can load.
fn face_with(bytes: &[u8], c: char) -> Option<u32> {
    use skrifa::MetadataProvider as _;
    (0..MAX_FACES).find(|&index| skrifa::FontRef::from_index(bytes, index).is_ok_and(|font| font.charmap().map(c).is_some()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broken_and_missing_files_are_skipped() {
        assert_eq!(face_with(b"", ARABIC), None);
        assert_eq!(face_with(b"not a font at all", ARABIC), None);
        assert_eq!(face_with(&[0u8; 4096], ARABIC), None);
        assert!(read(Path::new("definitely/not/here.ttf"), ARABIC, MAX_BYTES).is_none());
        // A directory is not a font.
        assert!(read(&std::env::temp_dir(), ARABIC, MAX_BYTES).is_none());
    }

    #[test]
    fn a_face_without_the_probe_is_rejected() {
        // Inter is Latin, Greek and Cyrillic only.
        let inter = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");
        assert_eq!(face_with(inter, ARABIC), None);
        assert_eq!(face_with(inter, HAN), None);
        assert_eq!(face_with(inter, 'A'), Some(0));
    }

    #[test]
    fn candidates_are_absolute_font_files() {
        let list = candidates();
        assert!(!list.is_empty());
        assert!(list.iter().all(|(p, _, _)| p.extension().is_some_and(|e| e == "ttf" || e == "ttc")));
    }
}

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
/// Faces tried in a collection (`.ttc`). PingFang.ttc has 24: four regions in six weights.
const MAX_FACES: u32 = 32;
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
    for candidate in candidates() {
        if found.iter().any(|&(p, _)| p == candidate.probe) {
            continue;
        }
        if let Some(data) = read(&candidate) {
            found.push((candidate.probe, data));
        }
    }
    found
}

/// One installed font file to try.
struct Candidate {
    path: PathBuf,
    /// The character a face must map to be accepted.
    probe: char,
    /// The size limit the file is checked against.
    max_bytes: u64,
    /// PostScript name of the one face wanted from a collection; `None` takes the first face
    /// that maps `probe`. A collection's first face is often not the one meant: PingFang.ttc
    /// starts with the Hong Kong variant, STHeiti's with the Traditional one.
    postscript: Option<&'static str>,
}

impl Candidate {
    fn new(path: impl Into<PathBuf>, probe: char, max_bytes: u64) -> Self {
        Self { path: path.into(), probe, max_bytes, postscript: None }
    }

    fn face(mut self, postscript: &'static str) -> Self {
        self.postscript = Some(postscript);
        self
    }
}

/// Well-known locations of faces with broad script coverage, best first.
fn candidates() -> Vec<Candidate> {
    if cfg!(windows) {
        let dir = std::env::var_os("WINDIR").or_else(|| std::env::var_os("SystemRoot")).map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        [("msyh.ttc", HAN, MAX_BYTES), ("segoeui.ttf", ARABIC, MAX_BYTES), ("tahoma.ttf", ARABIC, MAX_BYTES), ("arial.ttf", ARABIC, MAX_BYTES)]
            .iter()
            .map(|(f, probe, max_bytes)| Candidate::new(dir.join("Fonts").join(f), *probe, *max_bytes))
            .collect()
    } else if cfg!(target_os = "macos") {
        let fonts = Path::new("/System/Library/Fonts");
        // PingFang SC (苹方-简) Regular is the Chinese face. Older macOS keeps PingFang.ttc in
        // /System/Library/Fonts; macOS 26 moved it into a downloadable font asset.
        let pingfang = std::iter::once(fonts.join("PingFang.ttc")).chain(macos_font_assets("PingFang.ttc"));
        let mut list: Vec<Candidate> = pingfang.map(|path| Candidate::new(path, HAN, MAX_CHINESE_BYTES).face("PingFangSC-Regular")).collect();
        // Only if no PingFang is found: the system Chinese font before OS X 10.11.
        list.push(Candidate::new(fonts.join("STHeiti Light.ttc"), HAN, MAX_CHINESE_BYTES).face("STHeitiSC-Light"));
        list.push(Candidate::new(fonts.join("STHeiti Medium.ttc"), HAN, MAX_CHINESE_BYTES).face("STHeitiSC-Medium"));
        for f in ["SFArabic.ttf", "GeezaPro.ttc", "Supplemental/Arial.ttf"] {
            list.push(Candidate::new(fonts.join(f), ARABIC, MAX_BYTES));
        }
        list
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
            .flat_map(|dir| files.iter().map(move |(f, probe)| Candidate::new(Path::new(dir).join(f), *probe, MAX_BYTES)))
            .collect()
    }
}

/// `name` inside the downloadable font assets macOS 26 keeps under `/System/Library/AssetsV2`
/// (`com_apple_MobileAsset_Font<N>/<hash>.asset/AssetData/<name>`), newest asset set first. The
/// hash and `N` change between macOS releases, so the directories are listed, not hard-coded.
fn macos_font_assets(name: &str) -> Vec<PathBuf> {
    let children = |dir: &Path| -> Vec<PathBuf> { std::fs::read_dir(dir).into_iter().flatten().flatten().map(|entry| entry.path()).collect() };
    let mut sets: Vec<(u32, PathBuf)> = children(Path::new("/System/Library/AssetsV2"))
        .into_iter()
        .filter_map(|dir| {
            let version = dir.file_name()?.to_str()?.strip_prefix("com_apple_MobileAsset_Font")?.parse::<u32>().ok()?;
            Some((version, dir))
        })
        .collect();
    sets.sort_by_key(|(version, _)| std::cmp::Reverse(*version));
    sets.into_iter()
        .flat_map(|(_, set)| children(&set))
        .filter(|asset| asset.extension().is_some_and(|e| e == "asset"))
        .map(|asset| asset.join("AssetData").join(name))
        .collect()
}

fn read(candidate: &Candidate) -> Option<Arc<FontData>> {
    let Candidate { path, probe, max_bytes, postscript } = candidate;
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > *max_bytes {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let index = face_with(&bytes, *probe, *postscript)?;
    let mut data = FontData::from_owned(bytes);
    data.index = index;
    log::info!("interface font fallback: {} (face {index})", path.display());
    Some(Arc::new(data))
}

/// The first face of the file that parses, maps `c` and, when `postscript` is given, has that
/// PostScript name. egui parses fonts with the same skrifa, so a face accepted here is one it can
/// load.
fn face_with(bytes: &[u8], c: char, postscript: Option<&str>) -> Option<u32> {
    use skrifa::MetadataProvider as _;
    use skrifa::string::StringId;
    (0..MAX_FACES).find(|&index| {
        skrifa::FontRef::from_index(bytes, index).is_ok_and(|font| {
            font.charmap().map(c).is_some()
                && postscript.is_none_or(|want| font.localized_strings(StringId::POSTSCRIPT_NAME).any(|name| name.chars().eq(want.chars())))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn postscript_name(data: &FontData) -> String {
        use skrifa::MetadataProvider as _;
        let font = skrifa::FontRef::from_index(&data.font, data.index).expect("a face that was accepted parses");
        font.localized_strings(skrifa::string::StringId::POSTSCRIPT_NAME).next().map(|name| name.to_string()).unwrap_or_default()
    }

    #[test]
    fn broken_and_missing_files_are_skipped() {
        assert_eq!(face_with(b"", ARABIC, None), None);
        assert_eq!(face_with(b"not a font at all", ARABIC, None), None);
        assert_eq!(face_with(&[0u8; 4096], ARABIC, None), None);
        assert!(read(&Candidate::new("definitely/not/here.ttf", ARABIC, MAX_BYTES)).is_none());
        // A directory is not a font.
        assert!(read(&Candidate::new(std::env::temp_dir(), ARABIC, MAX_BYTES)).is_none());
    }

    #[test]
    fn a_face_without_the_probe_is_rejected() {
        // Inter is Latin, Greek and Cyrillic only.
        let inter = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");
        assert_eq!(face_with(inter, ARABIC, None), None);
        assert_eq!(face_with(inter, HAN, None), None);
        assert_eq!(face_with(inter, 'A', None), Some(0));
    }

    #[test]
    fn a_face_is_picked_by_its_postscript_name() {
        let inter = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");
        assert_eq!(face_with(inter, 'A', Some("Inter-Regular")), Some(0));
        // Right name, wrong character; right character, wrong name.
        assert_eq!(face_with(inter, HAN, Some("Inter-Regular")), None);
        assert_eq!(face_with(inter, 'A', Some("Inter-Bold")), None);
        assert_eq!(face_with(inter, 'A', Some("")), None);
    }

    #[test]
    fn candidates_are_absolute_font_files() {
        let list = candidates();
        assert!(!list.is_empty());
        assert!(list.iter().all(|c| c.path.extension().is_some_and(|e| e == "ttf" || e == "ttc")));
    }

    /// The scenario that went wrong on macOS 26: PingFang.ttc is not in /System/Library/Fonts any
    /// more, and its first face is the Hong Kong variant. Where PingFang is installed the Chinese
    /// fallback must be PingFang SC Regular, not STHeiti and not another region.
    #[test]
    fn chinese_fallback_is_pingfang_sc_where_installed() {
        if !cfg!(target_os = "macos") {
            return;
        }
        let pingfang_installed = candidates().iter().any(|c| c.postscript == Some("PingFangSC-Regular") && c.path.is_file());
        let han = fallback().into_iter().find(|&(probe, _)| probe == HAN);
        let Some((_, data)) = han else {
            eprintln!("skipping chinese_fallback_is_pingfang_sc_where_installed: no Han fallback loaded (or PDFCRAFT_SYSTEM_FONTS=0)");
            return;
        };
        if !pingfang_installed {
            eprintln!("skipping chinese_fallback_is_pingfang_sc_where_installed: PingFang is not installed here");
            return;
        }
        assert_eq!(postscript_name(&data), "PingFangSC-Regular");
    }

    /// The macOS 26 asset layout is found by listing directories: every path it returns ends in
    /// `AssetData/<name>` inside a `.asset` directory of a `com_apple_MobileAsset_Font<N>` set.
    #[test]
    fn macos_font_assets_have_the_asset_layout() {
        for path in macos_font_assets("PingFang.ttc") {
            assert!(path.ends_with("AssetData/PingFang.ttc"), "{path:?}");
            let asset = path.parent().and_then(Path::parent).expect("AssetData sits in an asset directory");
            assert!(asset.extension().is_some_and(|e| e == "asset"), "{path:?}");
            let set = asset.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()).expect("an asset sits in a set");
            assert!(set.starts_with("com_apple_MobileAsset_Font"), "{path:?}");
        }
    }
}

//! Path helpers shared by engines: safe writes and paths relative to a project file.

use std::io::Write;
use std::path::{Component, Path, PathBuf};

/// Write via `<file>.tmp` in the same folder, then rename: a crash never leaves half a file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Lower-case extension without the dot.
pub fn ext_lower(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// `a/./b/../c` → `a/c`, without touching the disk (the file may not exist yet).
pub fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// `rel` as written in a project file, resolved against the folder holding that file.
pub fn resolve(base_dir: &Path, rel: &str) -> PathBuf {
    let p = Path::new(rel);
    if p.is_absolute() {
        normalize(p)
    } else {
        normalize(&base_dir.join(p))
    }
}

/// `target` relative to `base_dir`, with forward slashes (stable in git across machines).
/// Another drive stays absolute.
pub fn relativize(base_dir: &Path, target: &Path) -> String {
    let base = normalize(base_dir);
    let target = normalize(target);
    let b: Vec<Component> = base.components().collect();
    let t: Vec<Component> = target.components().collect();
    let same_root = match (b.first(), t.first()) {
        (Some(x), Some(y)) => x.as_os_str().eq_ignore_ascii_case(y.as_os_str()),
        _ => false,
    };
    if !same_root || !target.is_absolute() {
        return target.to_string_lossy().replace('\\', "/");
    }
    let common = b.iter().zip(&t).take_while(|(x, y)| x.as_os_str().eq_ignore_ascii_case(y.as_os_str())).count();
    let mut parts: Vec<String> = std::iter::repeat_n("..".to_string(), b.len() - common).collect();
    parts.extend(t[common..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned()));
    if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_round_trip() {
        let base = if cfg!(windows) { Path::new("C:\\game\\art") } else { Path::new("/game/art") };
        let target = base.join("..").join("build").join("hero.png");
        let rel = relativize(base, &target);
        assert_eq!(rel, "../build/hero.png");
        assert_eq!(resolve(base, &rel), normalize(&target));
        assert_eq!(relativize(base, &base.join("hero")), "hero");
        assert_eq!(relativize(base, base), ".");
    }

    #[test]
    fn atomic_write_replaces() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("sub/out.json");
        write_atomic(&f, b"1").unwrap();
        write_atomic(&f, b"22").unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"22");
        assert!(!d.path().join("sub/out.json.tmp").exists());
    }
}

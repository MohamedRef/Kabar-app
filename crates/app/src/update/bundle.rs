//! Replacing Kabar.app on the Mac (`docs/RELEASE.md` §5): unzip the new
//! bundle next to the running one, swap the two with renames (so a failure
//! halfway puts the old one back), and reopen Kabar.
//!
//! The running process keeps its already-open binary, so the swap is safe
//! while Kabar runs; it only restarts when the learner says so, or at the
//! next launch in automatic mode.

use std::fs;
use std::path::{Path, PathBuf};

/// The `.app` this binary runs from (`Kabar.app/Contents/MacOS/kabar`), if any.
pub fn running_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    bundle_of(&exe)
}

fn bundle_of(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    (macos.file_name()? == "MacOS" && contents.file_name()? == "Contents" && app.extension()? == "app")
        .then(|| app.to_path_buf())
}

/// Unzips `zip` with `extract` into a folder beside `bundle`, checks it holds
/// one complete `.app`, and swaps it in.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn install_zip(
    zip: &Path,
    bundle: &Path,
    extract: impl FnOnce(&Path, &Path) -> Result<(), String>,
) -> Result<(), String> {
    let parent = bundle.parent().ok_or("Kabar.app has no folder")?;
    let stage = parent.join(format!(".kabar-update-{}", std::process::id()));
    let _ = fs::remove_dir_all(&stage);
    fs::create_dir_all(&stage).map_err(|e| format!("{}: {e}", stage.display()))?;
    let result = (|| {
        extract(zip, &stage)?;
        let apps: Vec<PathBuf> = fs::read_dir(&stage)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "app"))
            .collect();
        let [new] = apps.as_slice() else {
            return Err(format!("the zip holds {} apps, not one", apps.len()));
        };
        if !new.join("Contents/Info.plist").is_file() || !new.join("Contents/MacOS").is_dir() {
            return Err("the zip's Kabar.app is incomplete".into());
        }
        swap(bundle, new)
    })();
    let _ = fs::remove_dir_all(&stage);
    result
}

/// Puts `new` where `current` is; on failure the old bundle goes back.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn swap(current: &Path, new: &Path) -> Result<(), String> {
    let parent = current.parent().ok_or("no parent folder")?;
    let old = parent.join(format!(".Kabar-old-{}.app", std::process::id()));
    let _ = fs::remove_dir_all(&old);
    fs::rename(current, &old).map_err(|e| format!("{}: {e}", current.display()))?;
    if let Err(e) = fs::rename(new, current) {
        let _ = fs::rename(&old, current);
        return Err(format!("{}: {e}", current.display()));
    }
    let _ = fs::remove_dir_all(&old);
    Ok(())
}

/// `ditto -x -k`: the unzip that keeps a bundle's symlinks, modes and
/// signature intact (the feed's zip is made with `ditto -c -k --keepParent`).
#[cfg(target_os = "macos")]
pub fn ditto_extract(zip: &Path, into: &Path) -> Result<(), String> {
    let out = std::process::Command::new("/usr/bin/ditto")
        .arg("-x")
        .arg("-k")
        .arg(zip)
        .arg(into)
        .output()
        .map_err(|e| format!("ditto: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("ditto: {}", String::from_utf8_lossy(&out.stderr).trim()))
    }
}

/// Opens `bundle` again once this process has gone, then quits.
#[cfg(target_os = "macos")]
pub fn relaunch(bundle: &Path) -> ! {
    let _ = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg("sleep 1; /usr/bin/open -n \"$0\"")
        .arg(bundle)
        .spawn();
    std::process::exit(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_app(dir: &Path, marker: &str) -> PathBuf {
        let app = dir.join("Kabar.app");
        fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        fs::write(app.join("Contents/Info.plist"), "<plist/>").unwrap();
        fs::write(app.join("Contents/MacOS/kabar"), marker).unwrap();
        app
    }

    #[test]
    fn finds_the_bundle_from_its_binary() {
        let exe = Path::new("/Applications/Kabar.app/Contents/MacOS/kabar");
        assert_eq!(bundle_of(exe), Some(PathBuf::from("/Applications/Kabar.app")));
        assert_eq!(bundle_of(Path::new("/home/me/kabar-app/target/release/kabar")), None);
    }

    #[test]
    fn swaps_in_the_new_app_and_cleans_up() {
        let dir = tempfile::tempdir().unwrap();
        let apps = dir.path().join("Applications");
        let bundle = fake_app(&apps, "old");
        let staged = tempfile::tempdir().unwrap();
        let zip = dir.path().join("Kabar.app.zip");
        fs::write(&zip, b"zip").unwrap();
        install_zip(&zip, &bundle, |z, into| {
            assert_eq!(z, zip);
            let new = fake_app(staged.path(), "new");
            fs::rename(new, into.join("Kabar.app")).map_err(|e| e.to_string())
        })
        .unwrap();
        assert_eq!(fs::read_to_string(bundle.join("Contents/MacOS/kabar")).unwrap(), "new");
        let left: Vec<_> = fs::read_dir(&apps).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left, ["Kabar.app"], "no stage or old copy is left behind");
    }

    #[test]
    fn a_broken_zip_leaves_the_old_app_alone() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = fake_app(dir.path(), "old");
        let zip = dir.path().join("x.zip");
        let err = install_zip(&zip, &bundle, |_, into| {
            fs::create_dir_all(into.join("Kabar.app/Contents")).map_err(|e| e.to_string())
        })
        .unwrap_err();
        assert!(err.contains("incomplete"), "{err}");
        assert_eq!(fs::read_to_string(bundle.join("Contents/MacOS/kabar")).unwrap(), "old");
        assert!(install_zip(&zip, &bundle, |_, _| Err("bad zip".into())).is_err());
        assert_eq!(fs::read_to_string(bundle.join("Contents/MacOS/kabar")).unwrap(), "old");
    }

    /// The real unzip, on the Mac CI runner: a bundle zipped the way the
    /// feed is made comes back whole, symlinks included.
    #[cfg(target_os = "macos")]
    #[test]
    fn ditto_round_trip_installs() {
        use std::process::Command;
        let dir = tempfile::tempdir().unwrap();
        let build = tempfile::tempdir().unwrap();
        let new = fake_app(build.path(), "new");
        std::os::unix::fs::symlink("kabar", new.join("Contents/MacOS/link")).unwrap();
        let zip = dir.path().join("Kabar.app.zip");
        let ok = Command::new("/usr/bin/ditto")
            .args(["-c", "-k", "--keepParent"])
            .arg(&new)
            .arg(&zip)
            .status()
            .unwrap();
        assert!(ok.success());
        let bundle = fake_app(&dir.path().join("Applications"), "old");
        install_zip(&zip, &bundle, ditto_extract).unwrap();
        assert_eq!(fs::read_to_string(bundle.join("Contents/MacOS/kabar")).unwrap(), "new");
        assert!(fs::symlink_metadata(bundle.join("Contents/MacOS/link")).unwrap().file_type().is_symlink());
    }
}

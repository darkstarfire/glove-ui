//! The wallpaper of each user for the login screen. The login screen runs as another user,
//! who cannot read the home folders: each session keeps a copy of its wallpaper (already
//! reduced) in a shared folder, writable by everyone with the sticky bit (like /tmp: each
//! one only replaces their own file). The login screen only takes a copy that belongs to
//! the user it is named after.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// The shared folder (made by the installation, mode 1777).
pub const DIR: &str = "/var/lib/glove/wallpapers";

/// Where the copy of `login` goes.
pub fn path(login: &str) -> PathBuf {
    path_in(Path::new(DIR), login)
}

fn path_in(dir: &Path, login: &str) -> PathBuf {
    // A login never has a slash; anything odd is not a name of a file here.
    let safe: String = login.chars().filter(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-')).collect();
    dir.join(format!("{safe}.png"))
}

/// The copy of `login`, if there is one and it belongs to `uid` (no one else can put a
/// picture on another user's login).
pub fn of(login: &str, uid: u32) -> Option<PathBuf> {
    of_in(Path::new(DIR), login, uid)
}

fn of_in(dir: &Path, login: &str, uid: u32) -> Option<PathBuf> {
    let p = path_in(dir, login);
    let m = std::fs::symlink_metadata(&p).ok()?;
    (m.is_file() && m.uid() == uid).then_some(p)
}

/// Keeps `png` (the bytes of the picture) as the copy of `login`: written aside and then
/// renamed, so the login screen never reads half a file. Quietly nothing without the folder.
pub fn store(login: &str, png: &[u8]) -> std::io::Result<()> {
    store_in(Path::new(DIR), login, png)
}

fn store_in(dir: &Path, login: &str, png: &[u8]) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    let p = path_in(dir, login);
    let tmp = p.with_extension(format!("png.{}", std::process::id()));
    std::fs::write(&tmp, png)?;
    std::fs::rename(&tmp, &p)
}

/// The login of whoever is running this program (from /etc/passwd), for the copy's name.
pub fn current_login() -> Option<String> {
    let uid = unsafe { libc::getuid() };
    let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
    passwd.lines().find_map(|l| {
        let f: Vec<&str> = l.split(':').collect();
        (f.len() > 2 && f[2].parse::<u32>().ok() == Some(uid)).then(|| f[0].to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copia_so_do_dono() {
        let dir = std::env::temp_dir().join(format!("glove-lw-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        store_in(&dir, "ana", b"png").unwrap();
        let me = unsafe { libc::getuid() };
        assert_eq!(of_in(&dir, "ana", me), Some(dir.join("ana.png")));
        // Another uid: not taken.
        assert_eq!(of_in(&dir, "ana", me + 1), None);
        assert_eq!(path_in(&dir, "../etc/x"), dir.join("..etcx.png"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

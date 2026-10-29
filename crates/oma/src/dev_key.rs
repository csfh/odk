//! This computer's dev key (ADR 0085): the root and the release key `odk oma dev` signs its
//! builds with. Both are Ed25519 seeds in the state directory, made the first time they are
//! needed and never replaced, since a new root would leave every dev build's data under the old
//! one. Nothing but `odk oma dev` signs with them.

use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::bundle::Signing;
use crate::fetch::KeySet;
use crate::layout::Layout;
use crate::trust::{Context, OmaKeys, PublicKey, ReleaseKey, SigningKey, Timestamp};

/// How long a dev build's signature, and its release key in the key set, last: a year.
const WINDOW: i64 = 365 * 86_400;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot access {}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{} is not this computer's dev key: a seed is 32 bytes, and this is {len}; it is never replaced, so move it away to make a new one, and every dev build is installed again under it", path.display())]
    Length { path: PathBuf, len: usize },
    #[error("{} can be read by others (mode {mode:o}); `chmod 600` it", path.display())]
    Readable { path: PathBuf, mode: u32 },
    #[error("the clock is out of range")]
    Clock,
}

/// The dev root and the dev release key.
pub struct DevKey {
    pub root: SigningKey,
    pub release: SigningKey,
}

impl DevKey {
    /// Reads the dev key, making each seed that does not exist yet. The `bool` is whether the
    /// root was made now.
    pub fn load_or_create(layout: &Layout) -> Result<(Self, bool), Error> {
        let dir = layout.dev_key_dir();
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .map_err(|source| Error::Io {
                path: dir.clone(),
                source,
            })?;
        let (root, made) = seed(&dir.join("root"))?;
        let (release, _) = seed(&dir.join("release"))?;
        Ok((Self { root, release }, made))
    }

    /// What signs a build at `version`, issued at `now`, in Unix seconds.
    pub fn signing(&self, version: u64, now: i64) -> Result<Signing, Error> {
        Ok(Signing {
            release: self.release.clone(),
            root: self.root.public_key().key_id(),
            version,
            issued_at: at(now)?,
            expires_at: at(now + WINDOW)?,
            update_url: None,
        })
    }

    /// The dev root's key set at `version`, naming the dev release key for a year from `now`,
    /// and signed by the dev root, as a publisher would serve it.
    pub fn key_set(&self, version: u64, now: i64) -> Result<KeySet, Error> {
        let root = self.root.public_key();
        let release = self.release.public_key();
        let set = OmaKeys {
            root_public_key: root,
            root_key_id: root.key_id(),
            version,
            keys: vec![ReleaseKey {
                id: release.key_id(),
                not_after: at(now + WINDOW)?,
                public_key: release,
            }],
        };
        let body = set
            .to_canonical()
            .expect("a set of one key with a timestamp is canonical");
        let signature = self.root.sign(Context::KEYS, &body).0.to_vec();
        Ok(KeySet { body, signature })
    }
}

/// The dev root's public key, when this computer has one that can be read. It decides that a
/// root's key set is the kept one and is never fetched (ADR 0085), so a seed that cannot be read
/// is no dev root rather than an error for every other file.
pub fn root(layout: &Layout) -> Option<PublicKey> {
    read(&layout.dev_key_dir().join("root"))
        .ok()
        .map(|key| key.public_key())
}

fn at(seconds: i64) -> Result<Timestamp, Error> {
    Timestamp::from_unix_seconds(seconds).ok_or(Error::Clock)
}

/// The seed at `path`, made now if there is none: written beside it, then linked into place, so
/// a seed is whole when it appears and one made meanwhile is the one kept.
fn seed(path: &Path) -> Result<(SigningKey, bool), Error> {
    match read(path) {
        Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {}
        other => return other.map(|key| (key, false)),
    }
    let io = |path: &Path| {
        let path = path.to_owned();
        move |source| Error::Io { path, source }
    };
    let mut bytes = [0u8; 32];
    random(&mut bytes).map_err(io(path))?;
    let partial = path.with_extension(format!("partial-{}", std::process::id()));
    let _ = fs::remove_file(&partial);
    let written = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&partial)
        .and_then(|mut file| {
            file.write_all(&bytes)?;
            file.sync_all()
        })
        .map_err(io(&partial));
    let linked = written.and_then(|()| match fs::hard_link(&partial, path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(io(path)(e)),
    });
    let _ = fs::remove_file(&partial);
    if linked? {
        let key = SigningKey::from_seed(&bytes).expect("32 bytes");
        Ok((key, true))
    } else {
        read(path).map(|key| (key, false))
    }
}

fn read(path: &Path) -> Result<SigningKey, Error> {
    let io = |source| Error::Io {
        path: path.to_owned(),
        source,
    };
    let mode = fs::metadata(path).map_err(io)?.permissions().mode();
    if mode & 0o077 != 0 {
        return Err(Error::Readable {
            path: path.to_owned(),
            mode: mode & 0o777,
        });
    }
    let bytes = fs::read(path).map_err(io)?;
    SigningKey::from_seed(&bytes).ok_or(Error::Length {
        path: path.to_owned(),
        len: bytes.len(),
    })
}

/// Fills `bytes` from the kernel's random source, waiting until it is ready.
fn random(bytes: &mut [u8]) -> std::io::Result<()> {
    let mut filled = 0;
    while filled < bytes.len() {
        let rest = &mut bytes[filled..];
        // SAFETY: `rest` is valid for writes of its length.
        let n = unsafe { libc::getrandom(rest.as_mut_ptr().cast(), rest.len(), 0) };
        if n < 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() != std::io::ErrorKind::Interrupted {
                return Err(e);
            }
            continue;
        }
        filled += n as usize;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::trust::keys::{self, Signer};

    fn layout(dir: &TempDir) -> Layout {
        Layout::new(dir.path().join("home"), dir.path().join("data"))
    }

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn the_key_is_made_once_private_and_then_read() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        assert!(root(&layout).is_none());

        let (first, made) = DevKey::load_or_create(&layout).unwrap();
        assert!(made);
        let key_dir = layout.dev_key_dir();
        assert_eq!(mode(&key_dir), 0o700);
        for name in ["root", "release"] {
            assert_eq!(mode(&key_dir.join(name)), 0o600, "{name}");
            assert_eq!(fs::read(key_dir.join(name)).unwrap().len(), 32, "{name}");
        }
        assert_eq!(
            fs::read_dir(&key_dir).unwrap().count(),
            2,
            "nothing is left beside them"
        );
        assert_ne!(first.root.public_key(), first.release.public_key());

        let (again, made) = DevKey::load_or_create(&layout).unwrap();
        assert!(!made);
        assert_eq!(again.root.public_key(), first.root.public_key());
        assert_eq!(again.release.public_key(), first.release.public_key());
        assert_eq!(root(&layout), Some(first.root.public_key()));
    }

    #[test]
    fn a_seed_of_another_length_or_readable_by_others_is_refused_and_kept() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        DevKey::load_or_create(&layout).unwrap();
        let root_seed = layout.dev_key_dir().join("root");

        fs::write(&root_seed, [7u8; 31]).unwrap();
        let e = DevKey::load_or_create(&layout).err().unwrap();
        assert!(matches!(e, Error::Length { len: 31, .. }), "{e}");
        assert_eq!(fs::read(&root_seed).unwrap(), [7u8; 31], "never replaced");
        assert!(root(&layout).is_none());

        fs::write(&root_seed, [7u8; 32]).unwrap();
        fs::set_permissions(&root_seed, fs::Permissions::from_mode(0o640)).unwrap();
        let e = DevKey::load_or_create(&layout).err().unwrap();
        assert!(matches!(e, Error::Readable { mode: 0o640, .. }), "{e}");
        assert!(root(&layout).is_none());
    }

    #[test]
    fn the_key_set_names_the_release_key_for_a_year_under_the_root() {
        let dir = TempDir::new().unwrap();
        let (key, _) = DevKey::load_or_create(&layout(&dir)).unwrap();
        let now = 1_800_000_000;
        let set = key.key_set(9, now).unwrap();
        let root = key.root.public_key();
        let verified = keys::verify(&set.body, &set.signature, Signer::Pinned(&root)).unwrap();
        assert_eq!(verified.set().version, 9);
        let release = verified
            .set()
            .release_key(&key.release.public_key())
            .unwrap();
        assert_eq!(release.not_after.unix_seconds(), now + WINDOW);

        let signing = key.signing(3, now).unwrap();
        assert_eq!(signing.root, root.key_id());
        assert_eq!(signing.expires_at.unix_seconds(), now + WINDOW);
    }
}

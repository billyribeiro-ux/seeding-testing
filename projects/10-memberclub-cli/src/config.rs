//! Credentials storage for `memberclub login`.
//!
//! On Linux: `$XDG_CONFIG_HOME/memberclub/credentials.toml` (default
//! `~/.config/memberclub/credentials.toml`). The file is `chmod 600`
//! after every write so other users on a shared machine can't read it.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The shape of `credentials.toml`. Both tokens are kept so the CLI can
/// refresh transparently when the access token expires.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Credentials {
    pub api_base_url: String,
    pub access_token: String,
    pub refresh_token: String,
    /// RFC 3339 timestamp from the `expires_in` returned by /auth/login.
    /// Stored absolute so the CLI can decide whether to refresh without
    /// re-reading the JWT.
    pub access_expires_at: String,
    pub user_email: String,
}

/// Where the CLI looks for config + credentials. Carried as a value so
/// tests can construct one pointing at a tempdir.
#[derive(Debug, Clone)]
pub struct ConfigPaths {
    pub credentials: PathBuf,
}

impl ConfigPaths {
    /// The real on-disk paths derived from `directories::ProjectDirs`.
    /// Returns `None` if the platform has no notion of a config home
    /// (extremely rare; only triggers in containers without /home).
    pub fn from_system() -> Option<Self> {
        let dirs = directories::ProjectDirs::from("dev", "MemberClub", "memberclub")?;
        let dir = dirs.config_dir().to_path_buf();
        Some(Self {
            credentials: dir.join("credentials.toml"),
        })
    }

    /// Used by tests + dev — point the config at a specific directory.
    #[must_use]
    pub fn under(root: &Path) -> Self {
        Self {
            credentials: root.join("credentials.toml"),
        }
    }
}

/// Load the saved credentials from `paths.credentials`. Returns `None`
/// if the file doesn't exist (callers should treat that as "you need
/// to `memberclub login` first").
pub fn load(paths: &ConfigPaths) -> anyhow::Result<Option<Credentials>> {
    if !paths.credentials.exists() {
        return Ok(None);
    }
    let bytes = fs::read_to_string(&paths.credentials)?;
    let creds: Credentials = toml::from_str(&bytes)?;
    Ok(Some(creds))
}

/// Save credentials to disk with restrictive permissions. Creates the
/// parent directory if it doesn't exist yet.
pub fn save(paths: &ConfigPaths, creds: &Credentials) -> anyhow::Result<()> {
    if let Some(parent) = paths.credentials.parent() {
        fs::create_dir_all(parent)?;
    }
    let body = toml::to_string_pretty(creds)?;
    fs::write(&paths.credentials, body)?;
    chmod_user_only(&paths.credentials)?;
    Ok(())
}

/// Best-effort `chmod 600` (Unix only). On other platforms this is a
/// no-op — Windows ACLs are out of scope for this curriculum.
#[cfg(unix)]
fn chmod_user_only(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn chmod_user_only(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

/// Forget the saved credentials. Returns `Ok(true)` if a file was
/// removed, `Ok(false)` if there was nothing to delete (idempotent).
pub fn clear(paths: &ConfigPaths) -> anyhow::Result<bool> {
    if paths.credentials.exists() {
        fs::remove_file(&paths.credentials)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample() -> Credentials {
        Credentials {
            api_base_url: "https://api.memberclub.test".into(),
            access_token: "access.token.example".into(),
            refresh_token: "refresh.token.example".into(),
            access_expires_at: "2026-05-26T16:50:00Z".into(),
            user_email: "alice@example.test".into(),
        }
    }

    #[test]
    fn round_trip_save_then_load_returns_the_same_credentials() {
        let tmp = TempDir::new().unwrap();
        let paths = ConfigPaths::under(tmp.path());
        let creds = sample();
        save(&paths, &creds).unwrap();
        let back = load(&paths).unwrap();
        assert_eq!(back, Some(creds));
    }

    #[test]
    fn load_returns_none_when_file_is_missing() {
        let tmp = TempDir::new().unwrap();
        let paths = ConfigPaths::under(tmp.path());
        let back = load(&paths).unwrap();
        assert!(back.is_none());
    }

    #[test]
    fn clear_removes_the_file_and_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let paths = ConfigPaths::under(tmp.path());
        save(&paths, &sample()).unwrap();
        // First clear removes it.
        assert!(clear(&paths).unwrap());
        // Second clear is a no-op.
        assert!(!clear(&paths).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn save_sets_user_only_permissions() {
        use std::os::unix::fs::PermissionsExt as _;
        let tmp = TempDir::new().unwrap();
        let paths = ConfigPaths::under(tmp.path());
        save(&paths, &sample()).unwrap();
        let mode = fs::metadata(&paths.credentials)
            .unwrap()
            .permissions()
            .mode();
        // Strip the file-type bits; only the perm bits matter here.
        assert_eq!(
            mode & 0o777,
            0o600,
            "credentials must be chmod 600 after save"
        );
    }
}

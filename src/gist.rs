use anyhow::{Context, Result, bail};
use chrono::{DateTime, Datelike, Timelike, Utc};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const CURRENT: &str = "current";
const ID_LEN: usize = 26;

#[derive(Clone)]
pub struct Store {
    root: PathBuf,
}

pub struct Paste {
    pub id: String,
    pub path: PathBuf,
    pub body: String,
    pub modified: SystemTime,
}

impl Store {
    pub fn new() -> Result<Self> {
        Self::from_root(default_root()?)
    }

    pub fn from_root(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root)
            .with_context(|| format!("create gist directory {}", root.display()))?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn write(&self, body: &str) -> Result<Paste> {
        self.write_at(body, SystemTime::now())
    }

    pub fn current(&self) -> Result<Option<Paste>> {
        let current = self.root.join(CURRENT);
        let raw = match fs::read_to_string(&current) {
            Ok(raw) => raw,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err).with_context(|| format!("read {}", current.display())),
        };
        let id = raw.trim_end_matches('\n').trim_end_matches('\r');
        let path = self.path_for(id)?;
        if !path.is_file() {
            return Ok(None);
        }
        let body = fs::read_to_string(&path)
            .with_context(|| format!("read gist paste {}", path.display()))?;
        let modified = path
            .metadata()
            .with_context(|| format!("stat gist paste {}", path.display()))?
            .modified()
            .with_context(|| format!("read gist paste mtime {}", path.display()))?;

        Ok(Some(Paste {
            id: id.to_string(),
            path,
            body,
            modified,
        }))
    }

    fn write_at(&self, body: &str, created: SystemTime) -> Result<Paste> {
        let id = paste_id(created);
        let path = self.path_for(&id)?;
        fs::write(&path, body).with_context(|| format!("write gist paste {}", path.display()))?;
        set_mtime(&path, created)?;
        self.write_current(&id)?;
        self.current()?.context("missing written gist paste")
    }

    fn write_current(&self, id: &str) -> Result<()> {
        let tmp = self.root.join(format!("{CURRENT}.tmp"));
        fs::write(&tmp, format!("{id}\n"))
            .with_context(|| format!("write gist current pointer {}", tmp.display()))?;
        fs::rename(&tmp, self.root.join(CURRENT)).context("replace gist current pointer")
    }

    fn path_for(&self, id: &str) -> Result<PathBuf> {
        if !valid_id(id) {
            bail!("invalid gist paste id");
        }
        Ok(self.root.join(format!("{id}.txt")))
    }
}

pub(crate) fn default_root() -> Result<PathBuf> {
    Ok(crate::dirs::data()?.join("gist"))
}

fn paste_id(timestamp: SystemTime) -> String {
    let timestamp = DateTime::<Utc>::from(timestamp);
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}.{:09}Z",
        timestamp.year(),
        timestamp.month(),
        timestamp.day(),
        timestamp.hour(),
        timestamp.minute(),
        timestamp.second(),
        timestamp.nanosecond(),
    )
}

fn valid_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.len() != ID_LEN {
        return false;
    }
    bytes.iter().enumerate().all(|(idx, byte)| match idx {
        8 => *byte == b'T',
        15 => *byte == b'.',
        25 => *byte == b'Z',
        _ => byte.is_ascii_digit(),
    })
}

fn set_mtime(path: &Path, timestamp: SystemTime) -> Result<()> {
    let timestamp = filetime::FileTime::from_system_time(timestamp);
    filetime::set_file_mtime(path, timestamp)
        .with_context(|| format!("set gist paste mtime {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn default_root_uses_app_data_gist_dir() {
        let root = default_root().unwrap();

        assert_eq!(
            root.file_name().and_then(|name| name.to_str()),
            Some("gist")
        );
        assert_eq!(root.parent().unwrap(), crate::dirs::data().unwrap());
    }

    #[test]
    fn write_creates_timestamped_paste_and_current_pointer() {
        let td = TempDir::new("ghrm-gist-store");
        let store = Store::from_root(td.path().join("gist")).unwrap();
        let created = UNIX_EPOCH + Duration::new(0, 123_456_789);

        let paste = store.write_at("hello\nworld\n", created).unwrap();

        assert_eq!(paste.id, "19700101T000000.123456789Z");
        assert_eq!(paste.body, "hello\nworld\n");
        assert_eq!(paste.modified, created);
        assert_eq!(
            paste.path,
            store.root().join("19700101T000000.123456789Z.txt")
        );
        assert_eq!(
            fs::read_to_string(store.root().join(CURRENT)).unwrap(),
            "19700101T000000.123456789Z\n"
        );
    }

    #[test]
    fn current_returns_none_without_pointer() {
        let td = TempDir::new("ghrm-gist-current");
        let store = Store::from_root(td.path().join("gist")).unwrap();

        assert!(store.current().unwrap().is_none());
    }

    #[test]
    fn rejects_invalid_paste_ids() {
        let td = TempDir::new("ghrm-gist-path");
        let store = Store::from_root(td.path().join("gist")).unwrap();

        assert!(store.path_for("").is_err());
        assert!(store.path_for("../secret").is_err());
        assert!(store.path_for("19700101T000000.123456789Z").is_ok());
    }
}

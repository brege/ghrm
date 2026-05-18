use anyhow::{Context, Result, bail};
use chrono::{DateTime, Datelike, Timelike, Utc};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const CURRENT: &str = "current";
const ID_LEN: usize = 26;

#[derive(Clone)]
pub(crate) struct Store {
    root: PathBuf,
}

pub(crate) struct Paste {
    pub id: String,
    pub body: String,
}

pub(crate) struct Entry {
    pub id: String,
    pub name: String,
    pub modified: Option<u64>,
    pub size: Option<u64>,
    pub lines: Option<u64>,
    pub current: bool,
}

impl Store {
    pub(crate) fn new() -> Result<Self> {
        Self::from_root(default_root()?)
    }

    pub(crate) fn from_root(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root)
            .with_context(|| format!("create gist directory {}", root.display()))?;
        Ok(Self { root })
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn write(&self, body: &str) -> Result<Paste> {
        self.write_at(body, SystemTime::now())
    }

    pub(crate) fn current(&self) -> Result<Option<Paste>> {
        let Some(id) = self.current_id()? else {
            return Ok(None);
        };
        self.get(&id)
    }

    pub(crate) fn get(&self, id: &str) -> Result<Option<Paste>> {
        let path = self.path_for(id)?;
        if !path.is_file() {
            return Ok(None);
        }
        let body = fs::read_to_string(&path)
            .with_context(|| format!("read gist paste {}", path.display()))?;

        Ok(Some(Paste {
            id: id.to_string(),
            body,
        }))
    }

    pub(crate) fn entries(&self) -> Result<Vec<Entry>> {
        let current_id = self.current_id()?;
        let mut entries = Vec::new();
        for entry in fs::read_dir(&self.root)
            .with_context(|| format!("read gist directory {}", self.root.display()))?
        {
            let entry =
                entry.with_context(|| format!("read gist directory {}", self.root.display()))?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let Some(id) = name.strip_suffix(".txt") else {
                continue;
            };
            if !valid_id(id) {
                continue;
            }
            let meta = path
                .metadata()
                .with_context(|| format!("read gist paste metadata {}", path.display()))?;
            let body = fs::read_to_string(&path)
                .with_context(|| format!("read gist paste {}", path.display()))?;
            entries.push(Entry {
                id: id.to_string(),
                name: name.to_string(),
                modified: meta.modified().ok().and_then(system_time_secs),
                size: Some(meta.len()),
                lines: Some(body.lines().count() as u64),
                current: current_id.as_deref() == Some(id),
            });
        }
        entries.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(entries)
    }

    fn current_id(&self) -> Result<Option<String>> {
        let current = self.root.join(CURRENT);
        let raw = match fs::read_to_string(&current) {
            Ok(raw) => raw,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err).with_context(|| format!("read {}", current.display())),
        };
        let id = raw.trim_end_matches('\n').trim_end_matches('\r');
        self.path_for(id)?;
        Ok(Some(id.to_string()))
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

fn system_time_secs(timestamp: SystemTime) -> Option<u64> {
    timestamp
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
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
        let path = store.root().join("19700101T000000.123456789Z.txt");
        assert!(path.is_file());
        assert_eq!(path.metadata().unwrap().modified().unwrap(), created);
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
    fn entries_list_timestamped_pastes_newest_first_and_mark_current() {
        let td = TempDir::new("ghrm-gist-entries");
        let store = Store::from_root(td.path().join("gist")).unwrap();
        let first_body = "alpha\n";
        let second_body = "beta\ncharlie\n";

        store
            .write_at(first_body, UNIX_EPOCH + Duration::new(0, 1))
            .unwrap();
        store
            .write_at(second_body, UNIX_EPOCH + Duration::new(1, 2))
            .unwrap();
        fs::write(store.root().join("current.tmp"), "ignored\n").unwrap();
        fs::write(store.root().join("notes.json"), "{}").unwrap();
        fs::create_dir(store.root().join("19700101T000002.000000000Z.txt")).unwrap();

        let entries = store.entries().unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "19700101T000001.000000002Z");
        assert_eq!(entries[0].name, "19700101T000001.000000002Z.txt");
        assert_eq!(entries[0].size, Some(second_body.len() as u64));
        assert_eq!(entries[0].lines, Some(2));
        assert!(entries[0].current);
        assert_eq!(entries[1].id, "19700101T000000.000000001Z");
        assert!(!entries[1].current);
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

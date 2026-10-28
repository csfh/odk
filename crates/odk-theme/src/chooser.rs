//! What `odkd`'s file chooser takes and answers (ADR 0068): one request, as one line of JSON,
//! on `files.sock` beside `odkd.sock`, and one answer as one line.
//!
//! The application asks; `odkd` asks the desktop's portal outside the sandbox and answers with
//! the paths the user chose. A path is a name, not access: what the sandbox lets the application
//! read is unchanged.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Map, Value, json};

use crate::host::{VERSION, socket_path};

/// The file chooser's socket, beside `odkd.sock` in the same directory.
pub const SOCKET_NAME: &str = "files.sock";

/// A request is one line of at most this many bytes.
pub const MAX_REQUEST: usize = 16 * 1024;
/// A title, a label, a name or a pattern is at most this many characters.
pub const MAX_TEXT: usize = 256;
/// At most this many filters, each of at most [`MAX_PATTERNS`] patterns.
pub const MAX_FILTERS: usize = 16;
pub const MAX_PATTERNS: usize = 32;
/// At most this many dialogs are open at once, for every application together.
pub const MAX_OPEN: usize = 4;
/// How long `odkd` waits for the request after a client connects.
pub const REQUEST_WAIT: Duration = Duration::from_secs(5);
/// An answer is one line of at most this many bytes.
pub const MAX_ANSWER: u64 = 1024 * 1024;

/// The file chooser's socket of this environment: `files.sock` in the directory of
/// [`socket_path`], so `ODKD_SOCKET` moves both and `ODKD_SOCKET=none` turns both off.
pub fn files_socket_path() -> Option<PathBuf> {
    socket_path().map(|host| beside(&host))
}

/// The file chooser's socket beside the host socket `host`.
pub fn beside(host: &Path) -> PathBuf {
    host.with_file_name(SOCKET_NAME)
}

/// What the user is asked to choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChooserKind {
    /// One existing file, or several with `multiple`.
    Open,
    /// A file to write, which may not exist yet.
    Save,
    /// A folder, or several with `multiple`.
    Folder,
}

impl ChooserKind {
    fn name(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Save => "save",
            Self::Folder => "folder",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "open" => Self::Open,
            "save" => Self::Save,
            "folder" => Self::Folder,
            _ => return None,
        })
    }
}

/// Where the dialog starts. The named folders are the user's, which `odkd` looks up outside the
/// sandbox: inside it `$HOME` is the application's data directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartFolder {
    Home,
    Desktop,
    Documents,
    Downloads,
    Music,
    Pictures,
    Videos,
    /// An absolute path the application was given, such as one its daemon read.
    Path(PathBuf),
}

impl StartFolder {
    const NAMED: [(&'static str, Self); 7] = [
        ("home", Self::Home),
        ("desktop", Self::Desktop),
        ("documents", Self::Documents),
        ("downloads", Self::Downloads),
        ("music", Self::Music),
        ("pictures", Self::Pictures),
        ("videos", Self::Videos),
    ];

    /// The key of a named folder in `user-dirs.dirs`, such as `XDG_PICTURES_DIR`. `None` for the
    /// home folder and for a path.
    pub fn user_dirs_key(&self) -> Option<&'static str> {
        Some(match self {
            Self::Desktop => "XDG_DESKTOP_DIR",
            Self::Documents => "XDG_DOCUMENTS_DIR",
            Self::Downloads => "XDG_DOWNLOAD_DIR",
            Self::Music => "XDG_MUSIC_DIR",
            Self::Pictures => "XDG_PICTURES_DIR",
            Self::Videos => "XDG_VIDEOS_DIR",
            Self::Home | Self::Path(_) => return None,
        })
    }

    fn to_json(&self) -> Value {
        match self {
            Self::Path(path) => json!({ "path": path.to_string_lossy() }),
            named => {
                let name = Self::NAMED
                    .iter()
                    .find(|(_, f)| f == named)
                    .map(|(n, _)| *n);
                json!({ "named": name })
            }
        }
    }

    fn from_json(value: &Value) -> Result<Self, ChooserError> {
        if let Some(name) = value.get("named").and_then(Value::as_str) {
            return Self::NAMED
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, f)| f.clone())
                .ok_or_else(|| ChooserError::Invalid(format!("no folder named {name}")));
        }
        let path = value
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| ChooserError::Invalid("start_in is neither named nor a path".into()))?;
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err(ChooserError::Invalid(
                "a folder to start in is an absolute path".into(),
            ));
        }
        Ok(Self::Path(path))
    }
}

/// One name filter: a label such as "Images" and its glob patterns, such as `*.png`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFilter {
    pub label: String,
    pub patterns: Vec<String>,
}

/// What an application asks `odkd`'s file chooser for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChooserRequest {
    pub kind: ChooserKind,
    pub title: String,
    /// The label of the button that chooses, such as "Set background".
    pub accept_label: Option<String>,
    /// Several files or folders may be chosen; never for [`ChooserKind::Save`].
    pub multiple: bool,
    pub filters: Vec<FileFilter>,
    /// The file name a save dialog suggests.
    pub name: Option<String>,
    pub start_in: Option<StartFolder>,
}

impl ChooserRequest {
    /// A request of `kind` titled `title`, with nothing else.
    pub fn new(kind: ChooserKind, title: impl Into<String>) -> Self {
        Self {
            kind,
            title: title.into(),
            accept_label: None,
            multiple: false,
            filters: Vec::new(),
            name: None,
            start_in: None,
        }
    }

    /// The line a client writes, without its newline.
    pub fn to_line(&self) -> String {
        let mut line = Map::new();
        line.insert("v".into(), json!(VERSION));
        line.insert("kind".into(), json!(self.kind.name()));
        line.insert("title".into(), json!(self.title));
        if let Some(label) = &self.accept_label {
            line.insert("accept_label".into(), json!(label));
        }
        if self.multiple {
            line.insert("multiple".into(), json!(true));
        }
        if !self.filters.is_empty() {
            let filters: Vec<Value> = self
                .filters
                .iter()
                .map(|f| json!({ "label": f.label, "patterns": f.patterns }))
                .collect();
            line.insert("filters".into(), Value::Array(filters));
        }
        if let Some(name) = &self.name {
            line.insert("name".into(), json!(name));
        }
        if let Some(start) = &self.start_in {
            line.insert("start_in".into(), start.to_json());
        }
        Value::Object(line).to_string()
    }

    /// Reads a request and checks it against the bounds of ADR 0068.
    pub fn from_line(line: &str) -> Result<Self, ChooserError> {
        if line.len() > MAX_REQUEST {
            return Err(ChooserError::Invalid("the request is too long".into()));
        }
        let value: Value = serde_json::from_str(line)
            .map_err(|_| ChooserError::Invalid("the request is not JSON".into()))?;
        if value.get("v").and_then(Value::as_u64) != Some(VERSION) {
            return Err(ChooserError::Invalid(
                "the request is of another version".into(),
            ));
        }
        let text = |key: &str| -> Result<Option<String>, ChooserError> {
            match value.get(key) {
                None | Some(Value::Null) => Ok(None),
                Some(Value::String(s)) => bounded(key, s).map(|s| Some(s.to_owned())),
                Some(_) => Err(ChooserError::Invalid(format!("{key} is not text"))),
            }
        };
        let kind = value
            .get("kind")
            .and_then(Value::as_str)
            .and_then(ChooserKind::from_name)
            .ok_or_else(|| ChooserError::Invalid("no kind of open, save or folder".into()))?;
        let title = text("title")?.unwrap_or_default();
        let multiple = match value.get("multiple") {
            None => false,
            Some(Value::Bool(b)) => *b,
            Some(_) => {
                return Err(ChooserError::Invalid(
                    "multiple is not true or false".into(),
                ));
            }
        };
        if multiple && kind == ChooserKind::Save {
            return Err(ChooserError::Invalid(
                "a save dialog chooses one file".into(),
            ));
        }
        let filters = match value.get("filters") {
            None => Vec::new(),
            Some(Value::Array(filters)) => {
                if filters.len() > MAX_FILTERS {
                    return Err(ChooserError::Invalid("too many filters".into()));
                }
                filters.iter().map(filter).collect::<Result<_, _>>()?
            }
            Some(_) => return Err(ChooserError::Invalid("filters is not a list".into())),
        };
        let start_in = value
            .get("start_in")
            .map(StartFolder::from_json)
            .transpose()?;
        Ok(Self {
            kind,
            title,
            accept_label: text("accept_label")?,
            multiple,
            filters,
            name: text("name")?,
            start_in,
        })
    }
}

fn bounded<'a>(what: &str, s: &'a str) -> Result<&'a str, ChooserError> {
    if s.chars().count() > MAX_TEXT {
        return Err(ChooserError::Invalid(format!("{what} is too long")));
    }
    Ok(s)
}

fn filter(value: &Value) -> Result<FileFilter, ChooserError> {
    let label = value
        .get("label")
        .and_then(Value::as_str)
        .ok_or_else(|| ChooserError::Invalid("a filter has no label".into()))?;
    let patterns = value
        .get("patterns")
        .and_then(Value::as_array)
        .ok_or_else(|| ChooserError::Invalid("a filter has no patterns".into()))?;
    if patterns.is_empty() || patterns.len() > MAX_PATTERNS {
        return Err(ChooserError::Invalid(
            "a filter has no patterns or too many".into(),
        ));
    }
    let patterns = patterns
        .iter()
        .map(|p| {
            p.as_str()
                .ok_or_else(|| ChooserError::Invalid("a pattern is not text".into()))
                .and_then(|p| bounded("a pattern", p))
                .map(str::to_owned)
        })
        .collect::<Result<_, _>>()?;
    Ok(FileFilter {
        label: bounded("a filter's label", label)?.to_owned(),
        patterns,
    })
}

/// What `odkd` answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChooserAnswer {
    /// The paths the user chose, at least one.
    Chosen(Vec<PathBuf>),
    /// The user closed the dialog without choosing.
    Cancelled,
    /// No dialog was shown, and why: a request out of bounds, too many open, no portal.
    Failed(String),
}

impl ChooserAnswer {
    /// The line `odkd` writes, without its newline. A path that is not UTF-8 cannot be written
    /// in JSON and is left out; with none left, the answer is that the user cancelled.
    pub fn to_line(&self) -> String {
        let value = match self {
            Self::Chosen(paths) => {
                let paths: Vec<&str> = paths.iter().filter_map(|p| p.to_str()).collect();
                if paths.is_empty() {
                    json!({ "v": VERSION, "cancelled": true })
                } else {
                    json!({ "v": VERSION, "paths": paths })
                }
            }
            Self::Cancelled => json!({ "v": VERSION, "cancelled": true }),
            Self::Failed(why) => json!({ "v": VERSION, "error": why }),
        };
        value.to_string()
    }

    pub fn from_line(line: &str) -> Result<Self, ChooserError> {
        let value: Value = serde_json::from_str(line)
            .map_err(|_| ChooserError::Invalid("the answer is not JSON".into()))?;
        if value.get("v").and_then(Value::as_u64) != Some(VERSION) {
            return Err(ChooserError::Invalid(
                "the answer is of another version".into(),
            ));
        }
        if let Some(why) = value.get("error").and_then(Value::as_str) {
            return Ok(Self::Failed(why.to_owned()));
        }
        if value.get("cancelled").and_then(Value::as_bool) == Some(true) {
            return Ok(Self::Cancelled);
        }
        let paths = value
            .get("paths")
            .and_then(Value::as_array)
            .ok_or_else(|| ChooserError::Invalid("the answer has no paths".into()))?;
        let paths: Vec<PathBuf> = paths
            .iter()
            .filter_map(Value::as_str)
            .map(PathBuf::from)
            .collect();
        if paths.is_empty() {
            return Ok(Self::Cancelled);
        }
        Ok(Self::Chosen(paths))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChooserError {
    #[error("{0}")]
    Invalid(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn avatar() -> ChooserRequest {
        let mut r = ChooserRequest::new(ChooserKind::Open, "Choose a face");
        r.filters.push(FileFilter {
            label: "Images".into(),
            patterns: vec!["*.png".into(), "*.jpg".into(), "*.jpeg".into()],
        });
        r.start_in = Some(StartFolder::Pictures);
        r
    }

    #[test]
    fn a_request_reads_back_as_it_was_written() {
        let mut report = ChooserRequest::new(ChooserKind::Save, "Save diagnostic report");
        report.name = Some("omarchy-debug.txt".into());
        report.accept_label = Some("Save".into());
        report.start_in = Some(StartFolder::Path("/home/me/Reports".into()));
        let mut share = ChooserRequest::new(ChooserKind::Folder, "Share a folder");
        share.multiple = true;
        for r in [avatar(), report, share] {
            assert_eq!(ChooserRequest::from_line(&r.to_line()), Ok(r));
        }
    }

    #[test]
    fn a_request_out_of_bounds_is_refused() {
        let refused = |line: &str| ChooserRequest::from_line(line).is_err();
        assert!(refused("not json"));
        assert!(refused(r#"{"v":2,"kind":"open"}"#), "another version");
        assert!(refused(r#"{"v":1,"kind":"delete"}"#), "no such kind");
        assert!(refused(r#"{"v":1,"kind":"save","multiple":true}"#));
        assert!(refused(
            r#"{"v":1,"kind":"open","start_in":{"path":"relative"}}"#
        ));
        assert!(refused(
            r#"{"v":1,"kind":"open","start_in":{"named":"root"}}"#
        ));
        assert!(refused(
            r#"{"v":1,"kind":"open","filters":[{"label":"x","patterns":[]}]}"#
        ));
        let long = "x".repeat(MAX_TEXT + 1);
        assert!(refused(&format!(
            r#"{{"v":1,"kind":"open","title":"{long}"}}"#
        )));
        let mut many = avatar();
        many.filters = vec![many.filters[0].clone(); MAX_FILTERS + 1];
        assert!(refused(&many.to_line()));
        let padded = format!("{}{}", avatar().to_line(), " ".repeat(MAX_REQUEST));
        assert!(refused(&padded), "too long a line");
        let ok = ChooserRequest::from_line(r#"{"v":1,"kind":"open","extra":1}"#);
        assert!(ok.is_ok(), "keys it does not know are ignored");
    }

    #[test]
    fn an_answer_reads_back_and_a_path_that_is_not_text_is_left_out() {
        for a in [
            ChooserAnswer::Chosen(vec!["/home/me/face.png".into()]),
            ChooserAnswer::Cancelled,
            ChooserAnswer::Failed("the chooser is busy".into()),
        ] {
            assert_eq!(ChooserAnswer::from_line(&a.to_line()), Ok(a));
        }
        use std::os::unix::ffi::OsStrExt;
        let odd = PathBuf::from(std::ffi::OsStr::from_bytes(b"/home/me/\xff.png"));
        let line = ChooserAnswer::Chosen(vec![odd]).to_line();
        assert_eq!(
            ChooserAnswer::from_line(&line),
            Ok(ChooserAnswer::Cancelled)
        );
    }

    #[test]
    fn the_socket_is_beside_odkds() {
        assert_eq!(
            beside(Path::new("/run/user/1000/odk/odkd.sock")),
            PathBuf::from("/run/user/1000/odk/files.sock")
        );
    }
}

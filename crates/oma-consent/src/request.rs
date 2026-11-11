//! The request `odk-oma` writes on standard input (`docs/spec/consent-dialog.md`).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Request {
    pub name: String,
    pub identifier: String,
    pub version: String,
    #[serde(default)]
    pub buttons: Vec<String>,
    #[serde(default)]
    pub lines: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

const KNOWN: &[(&str, &str)] = &[
    ("install", "Install"),
    ("run-once", "Run once"),
    ("allow", "Allow"),
    ("keep-previous", "Keep previous version"),
    ("confirm", "It is a different publisher"),
    ("cancel", "Cancel"),
];

/// The words on the button for the answer `word`, when it is one the spec names.
pub fn label(word: &str) -> Option<&'static str> {
    KNOWN.iter().find(|(w, _)| *w == word).map(|(_, l)| *l)
}

impl Request {
    /// Reads and checks a request: it must be TOML and offer at least one button, each of them
    /// one the spec names.
    pub fn parse(text: &str) -> Result<Self, String> {
        let request: Request = toml::from_str(text).map_err(|e| format!("bad request: {e}"))?;
        if request.buttons.is_empty() || request.buttons.iter().any(|b| label(b).is_none()) {
            return Err("the request offers no usable buttons".to_owned());
        }
        Ok(request)
    }

    /// The window's height in points: room for the header, the lines, the notes and the buttons.
    pub fn height(&self) -> f32 {
        214.0 + 24.0 * self.lines.len() as f32 + 20.0 * self.notes.len() as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"
name = "Hello"
identifier = "dev.example.hello"
version = "1.0.0"
buttons = ["install", "run-once", "cancel"]
lines = ["Reads your Documents folder"]
"#;

    #[test]
    fn a_request_with_known_buttons_is_accepted() {
        let request = Request::parse(GOOD).unwrap();
        assert_eq!(request.buttons, ["install", "run-once", "cancel"]);
        assert_eq!(request.lines.len(), 1);
        assert!(request.notes.is_empty());
    }

    #[test]
    fn no_buttons_or_an_unknown_one_is_refused() {
        let none = GOOD.replace(r#"buttons = ["install", "run-once", "cancel"]"#, "");
        assert!(Request::parse(&none).is_err());
        let odd = GOOD.replace("run-once", "grant-everything");
        assert!(Request::parse(&odd).is_err());
    }

    #[test]
    fn text_that_is_not_a_request_is_refused() {
        assert!(Request::parse("this is not toml = = =").is_err());
        assert!(
            Request::parse("name = \"x\"").is_err(),
            "identifier and version are required"
        );
    }

    #[test]
    fn the_window_grows_with_its_lines_and_notes() {
        let mut request = Request::parse(GOOD).unwrap();
        let one = request.height();
        request.lines.push("more".into());
        request.notes.push("a note".into());
        assert_eq!(request.height(), one + 24.0 + 20.0);
    }
}

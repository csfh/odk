//! The `ui` and `network` capabilities and `app.subscribe` (ADR 0092, point 5), against Plugins
//! that behave and Plugins that do not. Nothing here touches the user's directories.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::rc::Rc;
use std::time::Duration;

use odk_plugin::{
    CallError, Data, ErrorKind, Grants, Host, Limits, MAX_BODY_BYTES, MAX_CONTRIBUTIONS,
    MAX_DESCRIPTION_BYTES, MAX_PENDING_REQUESTS, NetworkRequest, PluginError,
};

struct Fixture {
    root: tempfile::TempDir,
    heard: Rc<RefCell<Vec<PluginError>>>,
}

impl Fixture {
    fn new() -> Self {
        Self {
            root: tempfile::tempdir().unwrap(),
            heard: Rc::default(),
        }
    }

    fn plugin(&self, id: &str, capabilities: &[&str], main: &str) {
        let path = self.root.path().join("plugins").join(id);
        fs::create_dir_all(&path).unwrap();
        let caps: Vec<String> = capabilities.iter().map(|c| format!("\"{c}\"")).collect();
        fs::write(
            path.join("plugin.toml"),
            format!(
                "plugin_manifest_version = 1\nid = \"{id}\"\nname = \"{id}\"\nversion = \"1.0.0\"\nentry = \"main.luau\"\ncapabilities = [{}]\n",
                caps.join(", ")
            ),
        )
        .unwrap();
        fs::write(path.join("main.luau"), main).unwrap();
    }

    fn host(&self) -> Host {
        let heard = self.heard.clone();
        let mut host = Host::new(
            self.root.path(),
            Grants::all(),
            Limits {
                call_budget: Duration::from_millis(60),
                ..Limits::default()
            },
            move |e| heard.borrow_mut().push(e.clone()),
        );
        host.load_dir(&self.root.path().join("plugins"));
        host
    }
}

fn text(s: &str) -> Data {
    Data::Text(s.to_owned())
}

fn failed(result: Result<Data, CallError>) -> PluginError {
    match result {
        Err(CallError::Failed(e)) => e,
        other => panic!("expected a failure, got {other:?}"),
    }
}

const UI: &str = r#"
function add(point, description) return ui.add(point, description) end
function remove(id) return ui.remove(id) end
function open(id) opened = id; return id end
function opened_is() return opened end
function spin() while true do end end
function big(n) return ui.add("menu", { label = string.rep("x", n) }) end
function bad() return ui.add("menu", { label = function() end }) end
function mixed() return ui.add("menu", { 1, 2, label = "x" }) end
function not_a_table() return ui.add("menu", "text") end
"#;

#[test]
fn a_plugin_adds_a_description_to_a_point_the_application_registered_and_the_user_activates_it() {
    let f = Fixture::new();
    f.plugin("com.example.ui", &["ui"], UI);
    let mut host = f.host();
    host.register_point("menu");
    let id = match host
        .call(
            "com.example.ui",
            "add",
            &[
                text("menu"),
                Data::Map(BTreeMap::from([
                    ("label".to_owned(), text("Word count")),
                    ("action".to_owned(), text("open")),
                ])),
            ],
        )
        .unwrap()
    {
        Data::Number(n) => n as u64,
        other => panic!("{other:?}"),
    };
    let all = host.contributions("menu");
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].plugin, "com.example.ui");
    assert_eq!(all[0].id, id);
    assert_eq!(
        all[0].description,
        Data::Map(BTreeMap::from([
            ("label".to_owned(), text("Word count")),
            ("action".to_owned(), text("open")),
        ]))
    );
    assert!(host.contributions("other").is_empty());

    // The user uses it: the Plugin's own function is called with the identifier.
    assert_eq!(
        host.activate("com.example.ui", id),
        Ok(Data::Number(id as f64))
    );
    assert_eq!(
        host.call("com.example.ui", "opened_is", &[]),
        Ok(Data::Number(id as f64))
    );
    assert_eq!(
        host.activate("com.example.ui", 999),
        Err(CallError::NoSuchContribution(999))
    );

    // Removing it takes it away, once.
    assert_eq!(
        host.call("com.example.ui", "remove", &[Data::Number(id as f64)]),
        Ok(Data::Bool(true))
    );
    assert_eq!(
        host.call("com.example.ui", "remove", &[Data::Number(id as f64)]),
        Ok(Data::Bool(false))
    );
    assert!(host.contributions("menu").is_empty());
}

#[test]
fn a_plugin_cannot_make_an_extension_point_or_add_what_is_not_plain_data() {
    let f = Fixture::new();
    f.plugin("com.example.ui", &["ui"], UI);
    let mut host = f.host();
    host.register_point("menu");
    let map = || Data::Map(BTreeMap::from([("label".to_owned(), text("x"))]));

    let e = failed(host.call("com.example.ui", "add", &[text("made-up"), map()]));
    assert_eq!(e.kind, ErrorKind::Runtime);
    assert!(e.message.contains("no extension point"), "{}", e.message);

    for (function, why) in [
        ("bad", "function"),
        ("mixed", "keys"),
        ("not_a_table", "table with string keys"),
    ] {
        let e = failed(host.call("com.example.ui", function, &[]));
        assert!(
            e.message.contains(why) || e.message.to_lowercase().contains(why),
            "{function}: {}",
            e.message
        );
    }
    assert!(host.contributions("menu").is_empty(), "nothing was added");
}

#[test]
fn a_plugin_is_held_to_a_size_and_a_count_of_descriptions() {
    let f = Fixture::new();
    f.plugin("com.example.ui", &["ui"], UI);
    let mut host = f.host();
    host.register_point("menu");

    let e = failed(host.call(
        "com.example.ui",
        "big",
        &[Data::Number((MAX_DESCRIPTION_BYTES + 1) as f64)],
    ));
    assert!(e.message.contains("bytes of text"), "{}", e.message);
    assert!(
        host.call(
            "com.example.ui",
            "big",
            &[Data::Number(MAX_DESCRIPTION_BYTES as f64 - 5.0)]
        )
        .is_ok()
    );

    let one = || Data::Map(BTreeMap::from([("label".to_owned(), text("x"))]));
    // One is added already (the large one); fill up to the limit.
    for _ in 1..MAX_CONTRIBUTIONS {
        host.call("com.example.ui", "add", &[text("menu"), one()])
            .unwrap();
    }
    let e = failed(host.call("com.example.ui", "add", &[text("menu"), one()]));
    assert!(e.message.contains("descriptions"), "{}", e.message);
    assert_eq!(host.contributions("menu").len(), MAX_CONTRIBUTIONS);
}

#[test]
fn a_plugin_without_the_ui_capability_has_no_ui() {
    let f = Fixture::new();
    f.plugin(
        "com.example.plain",
        &[],
        "function seen() return { ui = ui ~= nil, network = network ~= nil } end",
    );
    let mut host = f.host();
    let Data::Map(seen) = host.call("com.example.plain", "seen", &[]).unwrap() else {
        panic!()
    };
    assert_eq!(seen["ui"], Data::Bool(false));
    assert_eq!(seen["network"], Data::Bool(false));
}

const NET: &str = r#"
answered = nil
function ask(request) return network.request(request) end
function cancel(id) return network.cancel(id) end
function on_network(id, ok, payload) answered = { id = id, ok = ok, payload = payload } end
function got() return answered end
function spin() while true do end end
"#;

fn request(fields: &[(&str, Data)]) -> Data {
    Data::Map(
        fields
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
    )
}

fn with_handler(f: &Fixture) -> (Host, Rc<RefCell<Vec<NetworkRequest>>>) {
    let host = f.host();
    let heard: Rc<RefCell<Vec<NetworkRequest>>> = Rc::default();
    let sink = heard.clone();
    host.on_network(move |r| sink.borrow_mut().push(r));
    (host, heard)
}

#[test]
fn a_request_goes_to_the_applications_handler_and_the_answer_comes_back_later() {
    let f = Fixture::new();
    f.plugin("com.example.net", &["network"], NET);
    let (mut host, heard) = with_handler(&f);

    let id = match host
        .call(
            "com.example.net",
            "ask",
            &[request(&[
                ("url", text("https://example.com/api")),
                ("method", text("post")),
                ("body", text("{}")),
            ])],
        )
        .unwrap()
    {
        Data::Number(n) => n as u64,
        other => panic!("{other:?}"),
    };
    // The handler heard it, checked and in one shape; the Plugin has not been answered.
    let heard_now = heard.borrow().clone();
    assert_eq!(heard_now.len(), 1);
    assert_eq!(heard_now[0].plugin, "com.example.net");
    assert_eq!(heard_now[0].id, id);
    assert_eq!(
        heard_now[0].request,
        request(&[
            ("url", text("https://example.com/api")),
            ("method", text("POST")),
            ("headers", Data::Map(BTreeMap::new())),
            ("body", text("{}")),
        ])
    );
    assert_eq!(host.call("com.example.net", "got", &[]), Ok(Data::Nil));

    // Later, the application answers.
    host.answer("com.example.net", id, Ok(text("hello")))
        .unwrap();
    let Data::Map(got) = host.call("com.example.net", "got", &[]).unwrap() else {
        panic!()
    };
    assert_eq!(got["id"], Data::Number(id as f64));
    assert_eq!(got["ok"], Data::Bool(true));
    assert_eq!(got["payload"], text("hello"));

    // An answer is heard once.
    assert_eq!(
        host.answer("com.example.net", id, Ok(text("again"))),
        Err(CallError::NoSuchRequest(id))
    );
}

#[test]
fn a_failed_request_is_answered_with_the_reason() {
    let f = Fixture::new();
    f.plugin("com.example.net", &["network"], NET);
    let (mut host, _) = with_handler(&f);
    let Data::Number(id) = host
        .call(
            "com.example.net",
            "ask",
            &[request(&[("url", text("https://example.com"))])],
        )
        .unwrap()
    else {
        panic!()
    };
    host.answer(
        "com.example.net",
        id as u64,
        Err("refused by the user".into()),
    )
    .unwrap();
    let Data::Map(got) = host.call("com.example.net", "got", &[]).unwrap() else {
        panic!()
    };
    assert_eq!(got["ok"], Data::Bool(false));
    assert_eq!(got["payload"], text("refused by the user"));
}

#[test]
fn a_request_the_host_would_not_make_is_refused_before_the_handler_hears_it() {
    let f = Fixture::new();
    f.plugin("com.example.net", &["network"], NET);
    let (mut host, heard) = with_handler(&f);
    let url = |u: &str| ("url", text(u));
    let cases: Vec<(Data, &str)> = vec![
        (request(&[url("file:///etc/passwd")]), "https://"),
        (
            request(&[url("https://example.com"), ("method", text("TRACE"))]),
            "method",
        ),
        (
            request(&[url("https://example.com"), ("verb", text("GET"))]),
            "no request field",
        ),
        (request(&[("method", text("GET"))]), "url"),
        (
            request(&[
                url("https://example.com"),
                (
                    "headers",
                    Data::Map(BTreeMap::from([(
                        "X-A".to_owned(),
                        text("a\r\nHost: evil"),
                    )])),
                ),
            ]),
            "not a valid header",
        ),
        (
            request(&[
                url("https://example.com"),
                ("body", Data::Text("x".repeat(MAX_BODY_BYTES + 1))),
            ]),
            "body",
        ),
        (text("https://example.com"), "table"),
    ];
    for (case, why) in cases {
        let e = failed(host.call("com.example.net", "ask", std::slice::from_ref(&case)));
        assert!(e.message.contains(why), "{case:?}: {}", e.message);
    }
    assert!(heard.borrow().is_empty(), "the handler heard none of them");
}

#[test]
fn with_no_handler_a_plugin_cannot_make_a_request() {
    let f = Fixture::new();
    f.plugin("com.example.net", &["network"], NET);
    let mut host = f.host();
    let e = failed(host.call(
        "com.example.net",
        "ask",
        &[request(&[("url", text("https://example.com"))])],
    ));
    assert!(e.message.contains("makes no requests"), "{}", e.message);
}

#[test]
fn a_plugin_may_have_only_so_many_requests_waiting_and_a_cancel_frees_one() {
    let f = Fixture::new();
    f.plugin("com.example.net", &["network"], NET);
    let (mut host, _) = with_handler(&f);
    let ask = |host: &mut Host| {
        host.call(
            "com.example.net",
            "ask",
            &[request(&[("url", text("https://example.com"))])],
        )
    };
    let mut ids = Vec::new();
    for _ in 0..MAX_PENDING_REQUESTS {
        let Data::Number(id) = ask(&mut host).unwrap() else {
            panic!()
        };
        ids.push(id);
    }
    let e = failed(ask(&mut host));
    assert!(e.message.contains("waiting"), "{}", e.message);
    assert_eq!(
        host.call("com.example.net", "cancel", &[Data::Number(ids[0])]),
        Ok(Data::Bool(true))
    );
    assert!(ask(&mut host).is_ok(), "one is free again");
    // A cancelled request cannot be answered.
    assert_eq!(
        host.answer("com.example.net", ids[0] as u64, Ok(Data::Nil)),
        Err(CallError::NoSuchRequest(ids[0] as u64))
    );
}

#[test]
fn a_plugin_that_runs_out_of_time_is_unloaded_with_what_it_added_and_asked() {
    let f = Fixture::new();
    f.plugin(
        "com.example.both",
        &["ui", "network", "app.state"],
        &format!("{UI}\n{NET}\napp.subscribe('theme', 'on_theme')\nfunction on_theme() end"),
    );
    let (mut host, _) = with_handler(&f);
    host.register_point("menu");
    host.call(
        "com.example.both",
        "add",
        &[text("menu"), request(&[("label", text("x"))])],
    )
    .unwrap();
    let Data::Number(id) = host
        .call(
            "com.example.both",
            "ask",
            &[request(&[("url", text("https://example.com"))])],
        )
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(host.contributions("menu").len(), 1);

    let e = failed(host.call("com.example.both", "spin", &[]));
    assert_eq!(e.kind, ErrorKind::Timeout);
    assert!(host.loaded().is_empty());
    assert!(
        host.contributions("menu").is_empty(),
        "its descriptions went"
    );
    assert_eq!(
        host.answer("com.example.both", id as u64, Ok(Data::Nil)),
        Err(CallError::NoSuchRequest(id as u64)),
        "its waiting request went"
    );
    assert_eq!(
        host.publish("theme", text("dark")),
        0,
        "its subscription went"
    );
}

#[test]
fn a_published_value_reaches_each_subscriber_and_one_that_fails_does_not_stop_the_rest() {
    let f = Fixture::new();
    f.plugin(
        "com.example.a-listens",
        &["app.state"],
        "seen = nil\napp.subscribe('theme', 'on_theme')\nfunction on_theme(name, value) seen = name .. '=' .. value end\nfunction last() return seen end",
    );
    f.plugin(
        "com.example.b-throws",
        &["app.state"],
        "app.subscribe('theme', 'on_theme')\nfunction on_theme() error('boom') end",
    );
    f.plugin(
        "com.example.c-listens",
        &["app.state"],
        "seen = nil\napp.subscribe('theme', 'on_theme')\napp.subscribe('other', 'on_other')\nfunction on_theme(name, value) seen = value end\nfunction on_other() end\nfunction last() return seen end",
    );
    let mut host = f.host();
    assert_eq!(host.publish("theme", text("dark")), 2, "two were called");
    assert_eq!(f.heard.borrow().len(), 1, "the one that threw was reported");
    assert_eq!(
        host.call("com.example.a-listens", "last", &[]),
        Ok(text("theme=dark"))
    );
    assert_eq!(
        host.call("com.example.c-listens", "last", &[]),
        Ok(text("dark"))
    );
    // A value that was published can be read, as one that was exposed.
    assert_eq!(host.publish("nobody-listens", Data::Nil), 0);
}

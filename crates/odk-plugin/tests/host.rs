//! The Plugin host, against Plugins that behave and Plugins that do not (ADR 0092). Nothing here
//! touches the user's directories: every Plugin lives in a throwaway one.

use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

use odk_plugin::{
    CallError, Capability, Data, ErrorKind, Grants, Host, Limits, Phase, PluginError,
};

type Heard = Rc<RefCell<Vec<PluginError>>>;

struct Fixture {
    root: tempfile::TempDir,
    heard: Heard,
}

impl Fixture {
    fn new() -> Self {
        Self {
            root: tempfile::tempdir().unwrap(),
            heard: Rc::default(),
        }
    }

    fn plugins(&self) -> std::path::PathBuf {
        self.root.path().join("plugins")
    }

    /// A Plugin directory named `dir` with the manifest `id`, `capabilities` and these files.
    fn plugin(&self, dir: &str, id: &str, capabilities: &[&str], files: &[(&str, &str)]) {
        let path = self.plugins().join(dir);
        fs::create_dir_all(&path).unwrap();
        let caps: Vec<String> = capabilities.iter().map(|c| format!("\"{c}\"")).collect();
        fs::write(
            path.join("plugin.toml"),
            format!(
                "plugin_manifest_version = 1\nid = \"{id}\"\nname = \"{dir}\"\nversion = \"1.0.0\"\nentry = \"main.luau\"\ncapabilities = [{}]\n",
                caps.join(", ")
            ),
        )
        .unwrap();
        for (name, content) in files {
            let file = path.join(name);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, content).unwrap();
        }
    }

    fn host(&self, grants: Grants, limits: Limits) -> Host {
        let heard = self.heard.clone();
        Host::new(self.root.path(), grants, limits, move |e| {
            heard.borrow_mut().push(e.clone());
        })
    }

    fn load(&self, host: &mut Host) -> Vec<(String, Result<String, PluginError>)> {
        host.load_dir(&self.plugins())
    }

    fn data(&self) -> &Path {
        self.root.path()
    }
}

fn quick() -> Limits {
    Limits {
        call_budget: Duration::from_millis(60),
        ..Limits::default()
    }
}

fn text(s: &str) -> Data {
    Data::Text(s.to_owned())
}

#[test]
fn a_plugin_loads_and_answers_a_call_with_plain_data() {
    let f = Fixture::new();
    f.plugin(
        "adder",
        "com.example.adder",
        &[],
        &[(
            "main.luau",
            "function add(a, b) return a + b end\nfunction shape(t) return { count = #t, first = t[1] } end",
        )],
    );
    let mut host = f.host(Grants::none(), Limits::default());
    let out = f.load(&mut host);
    assert_eq!(out[0].1.as_ref().unwrap(), "com.example.adder");
    assert_eq!(host.loaded(), ["com.example.adder"]);
    assert_eq!(
        host.call(
            "com.example.adder",
            "add",
            &[Data::Number(2.0), Data::Number(3.5)]
        ),
        Ok(Data::Number(5.5))
    );
    let got = host
        .call(
            "com.example.adder",
            "shape",
            &[Data::List(vec![text("x"), text("y")])],
        )
        .unwrap();
    let Data::Map(map) = got else {
        panic!("{got:?}")
    };
    assert_eq!(map["count"], Data::Number(2.0));
    assert_eq!(map["first"], text("x"));
    assert!(f.heard.borrow().is_empty());
}

#[test]
fn a_plugin_that_cannot_load_is_reported_and_the_others_load() {
    let f = Fixture::new();
    f.plugin("a-bad-manifest", "NOT AN ID", &[], &[("main.luau", "")]);
    f.plugin(
        "b-syntax",
        "com.example.syntax",
        &[],
        &[("main.luau", "function (")],
    );
    f.plugin(
        "c-throws",
        "com.example.throws",
        &[],
        &[("main.luau", "error('boom')")],
    );
    f.plugin("d-good", "com.example.good", &[], &[("main.luau", "x = 1")]);
    fs::create_dir_all(f.plugins().join("e-no-manifest")).unwrap();
    let mut host = f.host(Grants::none(), Limits::default());
    let out = f.load(&mut host);
    let kinds: Vec<_> = out
        .iter()
        .map(|(name, r)| (name.as_str(), r.as_ref().err().map(|e| e.kind)))
        .collect();
    assert_eq!(
        kinds,
        [
            ("a-bad-manifest", Some(ErrorKind::Manifest)),
            ("b-syntax", Some(ErrorKind::Syntax)),
            ("c-throws", Some(ErrorKind::Runtime)),
            ("d-good", None),
            ("e-no-manifest", Some(ErrorKind::Manifest)),
        ]
    );
    assert_eq!(host.loaded(), ["com.example.good"]);
    assert_eq!(f.heard.borrow().len(), 4, "each failure was reported once");
    assert!(f.heard.borrow().iter().all(|e| e.phase == Phase::Load));
}

#[test]
fn a_capability_the_host_refuses_means_the_plugin_is_not_loaded_and_the_host_is_told_which() {
    let f = Fixture::new();
    f.plugin(
        "greedy",
        "com.example.greedy",
        &["app.state", "network", "storage"],
        &[("main.luau", "")],
    );
    let mut host = f.host(Grants::of([Capability::AppState]), Limits::default());
    let out = f.load(&mut host);
    let err = out[0].1.as_ref().unwrap_err();
    assert_eq!(err.kind, ErrorKind::Refused);
    assert_eq!(err.refused, [Capability::Network, Capability::Storage]);
    assert!(err.message.contains("network") && err.message.contains("storage"));
    assert!(host.loaded().is_empty());
    assert_eq!(f.heard.borrow().len(), 1);
}

#[test]
fn only_the_granted_capabilities_exist_in_a_state() {
    let f = Fixture::new();
    let probe = "function seen() return { app = app ~= nil, storage = storage ~= nil, ui = ui ~= nil, network = network ~= nil, io = io ~= nil, exec = os ~= nil and os.execute ~= nil, package = package ~= nil } end";
    f.plugin("none", "com.example.none", &[], &[("main.luau", probe)]);
    f.plugin(
        "some",
        "com.example.some",
        &["app.state", "ui"],
        &[("main.luau", probe)],
    );
    let mut host = f.host(Grants::all(), Limits::default());
    f.load(&mut host);
    let mut seen = |id: &str| match host.call(id, "seen", &[]).unwrap() {
        Data::Map(m) => m
            .into_iter()
            .filter(|(_, v)| *v == Data::Bool(true))
            .map(|(k, _)| k)
            .collect::<Vec<_>>(),
        other => panic!("{other:?}"),
    };
    assert!(
        seen("com.example.none").is_empty(),
        "a plugin that asked for nothing has nothing"
    );
    assert_eq!(seen("com.example.some"), ["app", "ui"]);
}

#[test]
fn app_state_reads_only_what_the_host_exposed_and_cannot_list_it() {
    let f = Fixture::new();
    f.plugin(
        "reader",
        "com.example.reader",
        &["app.state"],
        &[(
            "main.luau",
            "function words() return app.get('words') end\nfunction secret() return app.get('secret') end\nfunction keys() local n = {} for k in pairs(app) do n[#n+1] = k end return n end",
        )],
    );
    let mut host = f.host(Grants::all(), Limits::default());
    host.expose("words", Data::Number(42.0));
    f.load(&mut host);
    assert_eq!(
        host.call("com.example.reader", "words", &[]),
        Ok(Data::Number(42.0))
    );
    assert_eq!(
        host.call("com.example.reader", "secret", &[]),
        Ok(Data::Nil)
    );
    host.expose("words", Data::Number(43.0));
    assert_eq!(
        host.call("com.example.reader", "words", &[]),
        Ok(Data::Number(43.0)),
        "live"
    );
    assert_eq!(
        host.call("com.example.reader", "keys", &[]),
        Ok(Data::List(vec![text("get")]))
    );
}

#[test]
fn plugins_do_not_share_globals() {
    let f = Fixture::new();
    f.plugin(
        "a",
        "com.example.a",
        &[],
        &[(
            "main.luau",
            "leak = 'from a'\nfunction mine() return leak end",
        )],
    );
    f.plugin(
        "b",
        "com.example.b",
        &[],
        &[("main.luau", "function theirs() return leak end")],
    );
    let mut host = f.host(Grants::none(), Limits::default());
    f.load(&mut host);
    assert_eq!(host.call("com.example.a", "mine", &[]), Ok(text("from a")));
    assert_eq!(host.call("com.example.b", "theirs", &[]), Ok(Data::Nil));
}

#[test]
fn a_plugin_that_never_ends_is_stopped_unloaded_and_reported_and_the_others_run() {
    let f = Fixture::new();
    f.plugin(
        "loop",
        "com.example.loop",
        &[],
        &[("main.luau", "function spin() while true do end end")],
    );
    f.plugin(
        "fine",
        "com.example.fine",
        &[],
        &[("main.luau", "function ok() return 'ok' end")],
    );
    let mut host = f.host(Grants::none(), quick());
    f.load(&mut host);
    let started = Instant::now();
    let err = host.call("com.example.loop", "spin", &[]).unwrap_err();
    let CallError::Failed(error) = err else {
        panic!("{err:?}")
    };
    assert_eq!(error.kind, ErrorKind::Timeout);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "stopped at its budget"
    );
    assert_eq!(
        host.loaded(),
        ["com.example.fine"],
        "unloaded for the session"
    );
    assert_eq!(
        host.call("com.example.loop", "spin", &[]),
        Err(CallError::Unloaded("com.example.loop".into()))
    );
    assert_eq!(host.call("com.example.fine", "ok", &[]), Ok(text("ok")));
    assert_eq!(f.heard.borrow().len(), 1);
    assert_eq!(f.heard.borrow()[0].kind, ErrorKind::Timeout);
}

#[test]
fn a_plugin_that_loops_while_loading_is_not_loaded() {
    let f = Fixture::new();
    f.plugin(
        "loop",
        "com.example.loop",
        &[],
        &[("main.luau", "while true do end")],
    );
    let mut host = f.host(Grants::none(), quick());
    let out = f.load(&mut host);
    assert_eq!(out[0].1.as_ref().unwrap_err().kind, ErrorKind::Timeout);
    assert!(host.loaded().is_empty());
}

#[test]
fn a_plugin_that_eats_memory_hits_its_limit_and_is_unloaded() {
    let f = Fixture::new();
    f.plugin(
        "hog",
        "com.example.hog",
        &[],
        &[(
            "main.luau",
            "function eat() local t = {} for i = 1, 50000000 do t[i] = {i} end return #t end",
        )],
    );
    let limits = Limits {
        memory: 4 * 1024 * 1024,
        call_budget: Duration::from_secs(5),
        ..Limits::default()
    };
    let mut host = f.host(Grants::none(), limits);
    f.load(&mut host);
    let err = host.call("com.example.hog", "eat", &[]).unwrap_err();
    let CallError::Failed(error) = err else {
        panic!("{err:?}")
    };
    assert_eq!(error.kind, ErrorKind::Memory);
    assert!(host.loaded().is_empty());
}

#[test]
fn an_error_in_a_call_is_reported_and_the_plugin_stays_loaded() {
    let f = Fixture::new();
    f.plugin(
        "flaky",
        "com.example.flaky",
        &[],
        &[("main.luau", "function bad() error('nope') end\nfunction good() return 1 end\nfunction fn() return function() end end")],
    );
    let mut host = f.host(Grants::none(), Limits::default());
    f.load(&mut host);
    let CallError::Failed(e) = host.call("com.example.flaky", "bad", &[]).unwrap_err() else {
        panic!()
    };
    assert_eq!((e.kind, e.phase), (ErrorKind::Runtime, Phase::Call));
    assert!(e.message.contains("nope"));
    assert_eq!(
        host.call("com.example.flaky", "good", &[]),
        Ok(Data::Number(1.0))
    );
    let CallError::Failed(e) = host.call("com.example.flaky", "fn", &[]).unwrap_err() else {
        panic!()
    };
    assert!(
        e.message
            .contains("only nil, booleans, numbers, strings and tables"),
        "{e}"
    );
    assert_eq!(host.loaded().len(), 1);
    assert_eq!(
        host.call("com.example.flaky", "missing", &[]),
        Err(CallError::NoSuchFunction("missing".into()))
    );
    assert_eq!(
        host.call("com.example.nobody", "x", &[]),
        Err(CallError::NoSuchPlugin("com.example.nobody".into()))
    );
}

#[test]
fn require_loads_a_module_of_the_plugin_once_and_nothing_outside_it() {
    let f = Fixture::new();
    f.plugin(
        "mods",
        "com.example.mods",
        &[],
        &[
            ("main.luau", "local util = require('lib/util')\nlocal again = require('lib/util')\nfunction same() return util == again end\nfunction count() return util.loads end\nfunction try(name) return pcall(require, name) end"),
            ("lib/util.luau", "loads = (loads or 0) + 1\nreturn { loads = loads }"),
        ],
    );
    let secret = f.data().join("outside.luau");
    fs::write(&secret, "return 'stolen'").unwrap();
    let mut host = f.host(Grants::none(), Limits::default());
    f.load(&mut host);
    assert_eq!(host.loaded(), ["com.example.mods"]);
    assert_eq!(
        host.call("com.example.mods", "same", &[]),
        Ok(Data::Bool(true))
    );
    assert_eq!(
        host.call("com.example.mods", "count", &[]),
        Ok(Data::Number(1.0)),
        "loaded once"
    );
    for name in [
        "../../outside",
        "../outside",
        "/etc/passwd",
        "./lib/util",
        "lib//util",
        "nothing-here",
        "@std/x",
    ] {
        let got = host.call("com.example.mods", "try", &[text(name)]).unwrap();
        assert_eq!(got, Data::Bool(false), "{name} was not refused");
    }
}

#[test]
fn require_will_not_follow_a_symlink_out_of_the_plugin() {
    let f = Fixture::new();
    f.plugin(
        "link",
        "com.example.link",
        &[],
        &[(
            "main.luau",
            "function try(name) return pcall(require, name) end",
        )],
    );
    let outside = f.data().join("outside.luau");
    fs::write(&outside, "return 'stolen'").unwrap();
    std::os::unix::fs::symlink(&outside, f.plugins().join("link/escape.luau")).unwrap();
    let mut host = f.host(Grants::none(), Limits::default());
    f.load(&mut host);
    assert_eq!(
        host.call("com.example.link", "try", &[text("escape")]),
        Ok(Data::Bool(false))
    );
}

#[test]
fn storage_is_the_plugins_own_directory_and_bounded() {
    let f = Fixture::new();
    f.plugin(
        "keeper",
        "com.example.keeper",
        &["storage"],
        &[(
            "main.luau",
            "function save(path, text) storage.write(path, text) end\nfunction load(path) return storage.read(path) end\nfunction names(path) return storage.list(path) end\nfunction drop(path) storage.remove(path) end\nfunction try(path) return pcall(storage.read, path) end\nfunction try_write(path, text) return pcall(storage.write, path, text) end",
        )],
    );
    let limits = Limits {
        storage: 64,
        ..Limits::default()
    };
    let mut host = f.host(Grants::all(), limits);
    f.load(&mut host);
    let id = "com.example.keeper";
    host.call(id, "save", &[text("notes/a.txt"), text("hello")])
        .unwrap();
    assert_eq!(
        host.call(id, "load", &[text("notes/a.txt")]),
        Ok(text("hello"))
    );
    assert_eq!(host.call(id, "load", &[text("missing")]), Ok(Data::Nil));
    assert_eq!(
        host.call(id, "names", &[text("notes")]),
        Ok(Data::List(vec![text("a.txt")]))
    );
    assert_eq!(
        host.call(id, "names", &[]),
        Ok(Data::List(vec![text("notes")]))
    );
    assert!(
        f.data()
            .join("plugin-data/com.example.keeper/notes/a.txt")
            .is_file(),
        "under plugin-data/<plugin id>/"
    );
    host.call(id, "drop", &[text("notes/a.txt")]).unwrap();
    assert_eq!(host.call(id, "load", &[text("notes/a.txt")]), Ok(Data::Nil));
    // The cap: 64 bytes in all.
    assert_eq!(
        host.call(id, "try_write", &[text("big"), Data::Text("x".repeat(65))]),
        Ok(Data::Bool(false))
    );
    host.call(id, "save", &[text("fits"), Data::Text("x".repeat(60))])
        .unwrap();
    assert_eq!(
        host.call(id, "try_write", &[text("more"), Data::Text("y".repeat(10))]),
        Ok(Data::Bool(false)),
        "the directory is nearly full"
    );
    host.call(id, "save", &[text("fits"), Data::Text("x".repeat(10))])
        .unwrap();
    host.call(id, "save", &[text("more"), Data::Text("y".repeat(10))])
        .unwrap();
}

#[test]
fn storage_refuses_a_path_that_leaves_the_plugins_directory() {
    let f = Fixture::new();
    f.plugin(
        "keeper",
        "com.example.keeper",
        &["storage"],
        &[(
            "main.luau",
            "function try_read(path) return pcall(storage.read, path) end\nfunction try_write(path) return pcall(storage.write, path, 'x') end\nfunction try_remove(path) return pcall(storage.remove, path) end",
        )],
    );
    fs::write(f.data().join("secret.txt"), "top secret").unwrap();
    let mut host = f.host(Grants::all(), Limits::default());
    f.load(&mut host);
    let outside = f.data().join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("file"), "x").unwrap();
    std::os::unix::fs::symlink(
        &outside,
        f.data().join("plugin-data/com.example.keeper/link"),
    )
    .unwrap();
    std::os::unix::fs::symlink(
        outside.join("file"),
        f.data().join("plugin-data/com.example.keeper/file-link"),
    )
    .unwrap();
    let created = outside.join("created-through-a-link");
    std::os::unix::fs::symlink(
        &created,
        f.data().join("plugin-data/com.example.keeper/dangling"),
    )
    .unwrap();
    let id = "com.example.keeper";
    assert_eq!(
        host.call(id, "try_write", &[text("dangling")]),
        Ok(Data::Bool(false))
    );
    assert!(
        !created.exists(),
        "a dangling link did not create a file outside"
    );
    for path in [
        "../../../secret.txt",
        "../secret.txt",
        "/etc/passwd",
        "link/file",
        "file-link",
        "a/../../b",
        "",
    ] {
        for function in ["try_read", "try_write", "try_remove"] {
            assert_eq!(
                host.call(id, function, &[text(path)]),
                Ok(Data::Bool(false)),
                "{function}({path:?}) was not refused"
            );
        }
    }
    assert_eq!(
        fs::read_to_string(f.data().join("secret.txt")).unwrap(),
        "top secret"
    );
    assert_eq!(
        fs::read_to_string(outside.join("file")).unwrap(),
        "x",
        "nothing was written through the link"
    );
}

#[test]
fn two_plugins_with_one_id_do_not_both_load() {
    let f = Fixture::new();
    f.plugin("one", "com.example.same", &[], &[("main.luau", "")]);
    f.plugin("two", "com.example.same", &[], &[("main.luau", "")]);
    let mut host = f.host(Grants::none(), Limits::default());
    let out = f.load(&mut host);
    assert!(out[0].1.is_ok());
    assert_eq!(out[1].1.as_ref().unwrap_err().kind, ErrorKind::Manifest);
    assert_eq!(host.loaded().len(), 1);
}

#[test]
fn a_table_that_is_too_deep_does_not_cross() {
    let f = Fixture::new();
    f.plugin(
        "deep",
        "com.example.deep",
        &[],
        &[("main.luau", "function deep() local t = {} for i = 1, 40 do t = { t } end return t end\nfunction mixed() return { 1, 2, x = 3 } end")],
    );
    let mut host = f.host(Grants::none(), Limits::default());
    f.load(&mut host);
    for function in ["deep", "mixed"] {
        let CallError::Failed(e) = host.call("com.example.deep", function, &[]).unwrap_err() else {
            panic!()
        };
        assert_eq!(e.kind, ErrorKind::Runtime, "{function}");
    }
    assert_eq!(
        host.loaded().len(),
        1,
        "a refused value is not a reason to unload"
    );
}

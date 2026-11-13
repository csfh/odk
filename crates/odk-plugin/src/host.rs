//! The Plugin host (ADR 0092): one sandboxed Luau state for each Plugin, holding only the
//! capabilities the host granted, with a memory limit and a time budget, and every failure
//! reported as a value.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use mlua::{Lua, MultiValue, Value, VmState};

use crate::data::Data;
use crate::extend::{
    Contribution, NetworkRequest, Shared, install_network, install_subscribe, install_ui,
};
use crate::manifest::{Capability, Manifest};
use crate::paths::{PathError, clean_relative, resolve_existing, resolve_new};

/// A state's memory limit, in bytes. Provisional (ADR 0092 leaves the values to measurement): an
/// empty sandboxed state used 436 584 bytes with `mlua` 0.12.2 on 2026-11-11, so this leaves a
/// Plugin about 7.5 MiB.
pub const DEFAULT_MEMORY_LIMIT: usize = 8 * 1024 * 1024;
/// The time one call into a Plugin may take, which the host's interface waits for. Provisional.
pub const DEFAULT_CALL_BUDGET: Duration = Duration::from_millis(100);
/// How many bytes a Plugin's private directory may hold. Provisional.
pub const DEFAULT_STORAGE_LIMIT: u64 = 1024 * 1024;
/// The largest Luau source file a Plugin may load. ODK's own.
const MAX_SOURCE: u64 = 1024 * 1024;

/// What a Plugin is held to. Each value is a named constant until measurement settles it.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub memory: usize,
    pub call_budget: Duration,
    pub storage: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            memory: DEFAULT_MEMORY_LIMIT,
            call_budget: DEFAULT_CALL_BUDGET,
            storage: DEFAULT_STORAGE_LIMIT,
        }
    }
}

/// The capabilities the host grants every Plugin that asks (ADR 0020).
#[derive(Debug, Clone, Default)]
pub struct Grants(BTreeSet<Capability>);

impl Grants {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn all() -> Self {
        Self(Capability::ALL.into_iter().collect())
    }

    pub fn of(capabilities: impl IntoIterator<Item = Capability>) -> Self {
        Self(capabilities.into_iter().collect())
    }

    fn refused(&self, wanted: &[Capability]) -> Vec<Capability> {
        wanted
            .iter()
            .copied()
            .filter(|c| !self.0.contains(c))
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Load,
    Call,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// `plugin.toml` could not be read or is invalid, or the Plugin's id is already loaded.
    Manifest,
    /// The host refused a capability the Plugin listed.
    Refused,
    Syntax,
    Runtime,
    /// The state hit its memory limit.
    Memory,
    /// A call took longer than its budget.
    Timeout,
}

/// A Plugin's failure, as a value (ADR 0092). It never stops the host or another Plugin.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("plugin {plugin}: {kind:?} while {phase:?}: {message}")]
pub struct PluginError {
    /// The Plugin's `id`, or its directory's name when its manifest could not be read.
    pub plugin: String,
    pub phase: Phase,
    pub kind: ErrorKind,
    pub message: String,
    /// The capabilities the host refused, for [`ErrorKind::Refused`].
    pub refused: Vec<Capability>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CallError {
    #[error("no Plugin has the id {0:?}")]
    NoSuchPlugin(String),
    /// It failed to load, or broke a limit and was unloaded for the session.
    #[error("the Plugin {0:?} is not loaded")]
    Unloaded(String),
    #[error("the Plugin has no function {0:?}")]
    NoSuchFunction(String),
    /// `Host::activate` for a description that is not there (removed, or its Plugin unloaded).
    #[error("no description has the identifier {0}")]
    NoSuchContribution(u64),
    /// `Host::answer` for a request that is not waiting.
    #[error("no request {0} is waiting")]
    NoSuchRequest(u64),
    #[error(transparent)]
    Failed(#[from] PluginError),
}

/// Whether a call is over its time, set by the interrupt that Luau runs inside its loops.
#[derive(Default)]
struct Clock {
    deadline: Cell<Option<Instant>>,
    expired: Cell<bool>,
}

struct Running {
    lua: Lua,
    clock: Rc<Clock>,
}

struct Plugin {
    manifest: Manifest,
    running: Option<Running>,
}

type Exposed = Rc<RefCell<BTreeMap<String, Data>>>;
type Report = Rc<dyn Fn(&PluginError)>;

pub struct Host {
    /// The application's data directory: Plugins load from `plugins/` in it and keep files in
    /// `plugin-data/<plugin id>/` (ADR 0092).
    data_dir: PathBuf,
    grants: Grants,
    limits: Limits,
    report: Report,
    exposed: Exposed,
    shared: Rc<Shared>,
    plugins: Vec<Plugin>,
}

impl Host {
    /// `data_dir` is the application's own data directory: `$HOME` inside its sandbox, which is
    /// `~/.local/share/oma/apps/<root key id>/<identifier>/data` on the host (ADR 0099).
    /// `report` hears every Plugin failure.
    pub fn new(
        data_dir: impl Into<PathBuf>,
        grants: Grants,
        limits: Limits,
        report: impl Fn(&PluginError) + 'static,
    ) -> Self {
        Self {
            data_dir: data_dir.into(),
            grants,
            limits,
            report: Rc::new(report),
            exposed: Rc::default(),
            shared: Rc::default(),
            plugins: Vec::new(),
        }
    }

    /// Makes `value` readable by name to a Plugin with `app.state`. There is no way for a Plugin
    /// to list names or to reach anything that was not exposed.
    pub fn expose(&self, name: impl Into<String>, value: Data) {
        self.exposed.borrow_mut().insert(name.into(), value);
    }

    /// Makes `value` readable as `expose` does, and calls the function of each Plugin that
    /// subscribed to `name` (`app.subscribe`) with the name and the value. A Plugin that fails is
    /// reported and the others are called all the same. Returns how many were called.
    pub fn publish(&mut self, name: &str, value: Data) -> usize {
        self.expose(name, value.clone());
        let mut called = 0;
        for (plugin, function) in self.shared.subscribers(name) {
            // A Plugin that was unloaded since has no state to call: it is skipped.
            if self
                .call(
                    &plugin,
                    &function,
                    &[Data::Text(name.to_owned()), value.clone()],
                )
                .is_ok()
            {
                called += 1;
            }
        }
        called
    }

    /// Registers an extension point a Plugin with `ui` may add descriptions to. Only the
    /// application can: a Plugin cannot make one.
    pub fn register_point(&self, name: impl Into<String>) {
        self.shared.register_point(name.into());
    }

    /// What the Plugins added to `point`, in the order they were added, for the application to
    /// draw with its own components. Nothing a Plugin gave is drawn by anyone else.
    pub fn contributions(&self, point: &str) -> Vec<Contribution> {
        self.shared.contributions(point)
    }

    /// The user used a contribution: calls the Plugin's global named by its description's
    /// `action` (text), with the contribution's identifier. A description with no `action` is
    /// only shown, and this returns `Ok(Nil)`.
    pub fn activate(&mut self, plugin: &str, contribution: u64) -> Result<Data, CallError> {
        let Some(found) = self.shared.contribution(plugin, contribution) else {
            return Err(CallError::NoSuchContribution(contribution));
        };
        let Data::Map(description) = &found.description else {
            return Ok(Data::Nil);
        };
        match description.get("action") {
            Some(Data::Text(action)) => {
                let action = action.clone();
                self.call(plugin, &action, &[Data::Number(contribution as f64)])
            }
            _ => Ok(Data::Nil),
        }
    }

    /// Sets the application's handler of Plugin requests (`network`). Without one, a request is an
    /// error the Plugin sees. The handler decides about a request, and answers it with
    /// [`Host::answer`] now or later; it must not block, and must not call into the host.
    pub fn on_network(&self, handler: impl Fn(NetworkRequest) + 'static) {
        self.shared.set_handler(Rc::new(handler));
    }

    /// Answers request `id` of `plugin`: calls the Plugin's global `on_network` with the identifier,
    /// whether it succeeded and the response (plain data) or the reason it failed (text). A
    /// request that is not waiting (answered already, cancelled, or its Plugin unloaded) is an
    /// error and calls nothing.
    pub fn answer(
        &mut self,
        plugin: &str,
        id: u64,
        response: Result<Data, String>,
    ) -> Result<Data, CallError> {
        if !self.shared.take_pending(plugin, id) {
            return Err(CallError::NoSuchRequest(id));
        }
        let (ok, payload) = match response {
            Ok(data) => (true, data),
            Err(reason) => (false, Data::Text(reason)),
        };
        self.call(
            plugin,
            "on_network",
            &[Data::Number(id as f64), Data::Bool(ok), payload],
        )
    }

    /// Where the Plugins are loaded from by default.
    pub fn plugins_dir(&self) -> PathBuf {
        self.data_dir.join("plugins")
    }

    /// The ids of the Plugins that are loaded, in the order they loaded.
    pub fn loaded(&self) -> Vec<&str> {
        self.plugins
            .iter()
            .filter(|p| p.running.is_some())
            .map(|p| p.manifest.id.as_str())
            .collect()
    }

    /// Loads every subdirectory of `plugins_dir` that holds a Plugin, in name order, at start.
    /// A Plugin that cannot load is reported and left out; the others load. Returns each
    /// directory's outcome: the Plugin's id, or why it did not load.
    pub fn load_dir(&mut self, plugins_dir: &Path) -> Vec<(String, Result<String, PluginError>)> {
        let mut dirs: Vec<PathBuf> = fs::read_dir(plugins_dir)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .collect()
            })
            .unwrap_or_default();
        dirs.sort();
        dirs.into_iter()
            .map(|dir| {
                let name = dir
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let outcome = self.load(&dir, &name);
                if let Err(e) = &outcome {
                    (self.report)(e);
                }
                (name, outcome)
            })
            .collect()
    }

    fn load(&mut self, dir: &Path, dir_name: &str) -> Result<String, PluginError> {
        let fail = |plugin: &str, kind, message: String| PluginError {
            plugin: plugin.to_owned(),
            phase: Phase::Load,
            kind,
            message,
            refused: Vec::new(),
        };
        let text = fs::read_to_string(dir.join("plugin.toml")).map_err(|e| {
            fail(
                dir_name,
                ErrorKind::Manifest,
                format!("cannot read plugin.toml: {e}"),
            )
        })?;
        let manifest = Manifest::parse(&text)
            .map_err(|e| fail(dir_name, ErrorKind::Manifest, e.to_string()))?;
        let id = manifest.id.clone();
        if self.plugins.iter().any(|p| p.manifest.id == id) {
            return Err(fail(
                &id,
                ErrorKind::Manifest,
                "another Plugin has this id".into(),
            ));
        }
        let refused = self.grants.refused(&manifest.capabilities);
        if !refused.is_empty() {
            let names: Vec<_> = refused.iter().map(|c| c.name()).collect();
            return Err(PluginError {
                refused,
                ..fail(
                    &id,
                    ErrorKind::Refused,
                    format!("the host refused {}", names.join(", ")),
                )
            });
        }
        match self.start(&manifest, dir) {
            Ok(running) => {
                self.plugins.push(Plugin {
                    manifest,
                    running: Some(running),
                });
                Ok(id)
            }
            Err((kind, message)) => Err(fail(&id, kind, message)),
        }
    }

    /// Builds the Plugin's state and runs its entry file.
    fn start(&self, manifest: &Manifest, dir: &Path) -> Result<Running, (ErrorKind, String)> {
        let clock = Rc::new(Clock::default());
        let lua = Lua::new();
        let as_error =
            |e: mlua::Error, clock: &Clock| (classify(&e, clock.expired.get()), e.to_string());
        let setup = || -> mlua::Result<()> {
            lua.sandbox(true)?;
            lua.set_memory_limit(self.limits.memory)?;
            let watch = clock.clone();
            lua.set_interrupt(move |_| match watch.deadline.get() {
                Some(deadline) if Instant::now() > deadline => {
                    watch.expired.set(true);
                    Err(mlua::Error::runtime("the Plugin ran past its time budget"))
                }
                _ => Ok(VmState::Continue),
            });
            install_require(&lua, dir)?;
            self.install_capabilities(&lua, manifest)?;
            Ok(())
        };
        setup().map_err(|e| as_error(e, &clock))?;
        let entry = resolve_existing(dir, &manifest.entry)
            .map_err(|e| (ErrorKind::Manifest, format!("entry: {e}")))?;
        let source =
            read_source(&entry).map_err(|m| (ErrorKind::Manifest, format!("entry: {m}")))?;
        budgeted(&clock, self.limits.call_budget, || {
            lua.load(source.as_str())
                .set_name(format!("={}", manifest.entry))
                .exec()
        })
        .map_err(|e| as_error(e, &clock))?;
        Ok(Running { lua, clock })
    }

    fn install_capabilities(&self, lua: &Lua, manifest: &Manifest) -> mlua::Result<()> {
        for capability in &manifest.capabilities {
            let table = lua.create_table()?;
            match capability {
                Capability::AppState => {
                    let exposed = self.exposed.clone();
                    table.set(
                        "get",
                        lua.create_function(move |lua, name: String| {
                            match exposed.borrow().get(&name) {
                                Some(value) => value.to_lua(lua),
                                None => Ok(Value::Nil),
                            }
                        })?,
                    )?;
                    install_subscribe(lua, &table, manifest.id.clone(), self.shared.clone())?;
                    lua.globals().set("app", table)?;
                }
                Capability::Storage => {
                    let root = self.data_dir.join("plugin-data").join(&manifest.id);
                    fs::create_dir_all(&root).map_err(mlua::Error::external)?;
                    install_storage(lua, &table, root, self.limits.storage)?;
                    lua.globals().set("storage", table)?;
                }
                Capability::Ui => {
                    install_ui(lua, &table, manifest.id.clone(), self.shared.clone())?;
                    lua.globals().set("ui", table)?;
                }
                Capability::Network => {
                    install_network(lua, &table, manifest.id.clone(), self.shared.clone())?;
                    lua.globals().set("network", table)?;
                }
            }
        }
        Ok(())
    }

    /// Calls the global `function` of the Plugin `id` with plain data, within the call budget.
    /// A failure is reported to the host's callback and returned. A Plugin that breaks its memory
    /// limit or its time budget is unloaded for the rest of the session (ADR 0092, point 6).
    pub fn call(&mut self, id: &str, function: &str, args: &[Data]) -> Result<Data, CallError> {
        let budget = self.limits.call_budget;
        let report = self.report.clone();
        let shared = self.shared.clone();
        let plugin = self
            .plugins
            .iter_mut()
            .find(|p| p.manifest.id == id)
            .ok_or_else(|| CallError::NoSuchPlugin(id.to_owned()))?;
        let running = plugin
            .running
            .as_ref()
            .ok_or_else(|| CallError::Unloaded(id.to_owned()))?;
        let found: Value = running
            .lua
            .globals()
            .get(function)
            .map_err(|e| failure(id, &e, false))?;
        let Value::Function(f) = found else {
            return Err(CallError::NoSuchFunction(function.to_owned()));
        };
        let outcome = (|| -> mlua::Result<Result<Data, String>> {
            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                values.push(arg.to_lua(&running.lua)?);
            }
            let returned: Value = budgeted(&running.clock, budget, || {
                f.call(MultiValue::from_iter(values))
            })?;
            Ok(Data::from_lua(&returned))
        })();
        let error = match outcome {
            Ok(Ok(data)) => return Ok(data),
            Ok(Err(message)) => PluginError {
                plugin: id.to_owned(),
                phase: Phase::Call,
                kind: ErrorKind::Runtime,
                message: format!("{function} returned {message}"),
                refused: Vec::new(),
            },
            Err(e) => match failure(id, &e, running.clock.expired.get()) {
                CallError::Failed(error) => error,
                other => return Err(other),
            },
        };
        if matches!(error.kind, ErrorKind::Memory | ErrorKind::Timeout) {
            plugin.running = None;
            // What it added, asked and subscribed to goes with it (ADR 0092, point 6).
            shared.forget(id);
        }
        report(&error);
        Err(CallError::Failed(error))
    }
}

/// Runs `f` with the call's deadline set, and clears it after.
fn budgeted<R>(
    clock: &Clock,
    budget: Duration,
    f: impl FnOnce() -> mlua::Result<R>,
) -> mlua::Result<R> {
    clock.expired.set(false);
    clock.deadline.set(Some(Instant::now() + budget));
    let result = f();
    clock.deadline.set(None);
    result
}

fn failure(plugin: &str, e: &mlua::Error, expired: bool) -> CallError {
    CallError::Failed(PluginError {
        plugin: plugin.to_owned(),
        phase: Phase::Call,
        kind: classify(e, expired),
        message: e.to_string(),
        refused: Vec::new(),
    })
}

fn classify(e: &mlua::Error, expired: bool) -> ErrorKind {
    if expired {
        return ErrorKind::Timeout;
    }
    match e {
        mlua::Error::MemoryError(_) => ErrorKind::Memory,
        mlua::Error::SyntaxError { .. } => ErrorKind::Syntax,
        mlua::Error::CallbackError { cause, .. } => classify(cause, false),
        _ => ErrorKind::Runtime,
    }
}

fn read_source(path: &Path) -> Result<String, String> {
    let size = fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > MAX_SOURCE {
        return Err(format!(
            "the file is {size} bytes, and a source file may be {MAX_SOURCE}"
        ));
    }
    fs::read_to_string(path).map_err(|e| e.to_string())
}

/// Replaces `require` (ADR 0092, point 4): it loads only `<name>.luau` inside the Plugin's
/// directory, by the rule of `entry`, once for each state.
fn install_require(lua: &Lua, dir: &Path) -> mlua::Result<()> {
    let dir = dir.to_path_buf();
    let loaded = lua.create_table()?;
    let require = lua.create_function(move |lua, name: String| {
        let cached: Value = loaded.raw_get(name.as_str())?;
        if !cached.is_nil() {
            return Ok(cached);
        }
        let file = format!("{name}.luau");
        if clean_relative(&file).is_none() {
            return Err(mlua::Error::runtime(format!(
                "require {name:?}: a module is a relative name with no `..` inside the Plugin's directory"
            )));
        }
        let path = resolve_existing(&dir, &file)
            .map_err(|e| mlua::Error::runtime(format!("require {name:?}: {e}")))?;
        let source = read_source(&path)
            .map_err(|e| mlua::Error::runtime(format!("require {name:?}: {e}")))?;
        let value: Value = lua.load(source.as_str()).set_name(format!("={file}")).eval()?;
        let value = if value.is_nil() { Value::Boolean(true) } else { value };
        loaded.raw_set(name.as_str(), value.clone())?;
        Ok(value)
    })?;
    lua.globals().set("require", require)
}

fn install_storage(lua: &Lua, table: &mlua::Table, root: PathBuf, cap: u64) -> mlua::Result<()> {
    let err = |what: &str, e: PathError| mlua::Error::runtime(format!("storage.{what}: {e}"));
    let io = |what: &str, e: std::io::Error| mlua::Error::runtime(format!("storage.{what}: {e}"));

    let r = root.clone();
    table.set(
        "read",
        lua.create_function(move |lua, path: String| {
            let file = match resolve_existing(&r, &path) {
                Ok(file) => file,
                Err(PathError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(Value::Nil);
                }
                Err(e) => return Err(err("read", e)),
            };
            let bytes = fs::read(file).map_err(|e| io("read", e))?;
            Ok(Value::String(lua.create_string(bytes)?))
        })?,
    )?;

    let r = root.clone();
    table.set(
        "write",
        lua.create_function(move |_, (path, content): (String, mlua::LuaString)| {
            let bytes = content.as_bytes();
            let file = resolve_new(&r, &path).map_err(|e| err("write", e))?;
            let before = fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
            let after = dir_size(&r).saturating_sub(before) + bytes.len() as u64;
            if after > cap {
                return Err(mlua::Error::runtime(format!(
                    "storage.write: the Plugin's directory may hold {cap} bytes, and this would make it {after}"
                )));
            }
            if let Some(parent) = file.parent() {
                fs::create_dir_all(parent).map_err(|e| io("write", e))?;
            }
            fs::write(file, &*bytes).map_err(|e| io("write", e))
        })?,
    )?;

    let r = root.clone();
    table.set(
        "list",
        lua.create_function(move |lua, path: Option<String>| {
            let dir = match path {
                Some(p) => resolve_existing(&r, &p).map_err(|e| err("list", e))?,
                None => fs::canonicalize(&r).map_err(|e| io("list", e))?,
            };
            let mut names: Vec<String> = fs::read_dir(dir)
                .map_err(|e| io("list", e))?
                .filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            lua.create_sequence_from(names)
        })?,
    )?;

    table.set(
        "remove",
        lua.create_function(move |_, path: String| {
            let file = resolve_existing(&root, &path).map_err(|e| err("remove", e))?;
            if !file.is_file() {
                return Err(mlua::Error::runtime(
                    "storage.remove: only a file is removed",
                ));
            }
            fs::remove_file(file).map_err(|e| io("remove", e))
        })?,
    )?;
    Ok(())
}

/// The bytes the files under `dir` hold.
fn dir_size(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| match e.metadata() {
                    Ok(m) if m.is_dir() => dir_size(&e.path()),
                    Ok(m) => m.len(),
                    Err(_) => 0,
                })
                .sum()
        })
        .unwrap_or(0)
}

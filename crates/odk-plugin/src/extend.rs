//! What a Plugin may ask of the application beyond reading and storing: the `ui` capability, which
//! adds descriptions to extension points the application registered, the `network` capability,
//! which hands a request to the application and hears the answer later, and `app.subscribe`,
//! which names a function to call when the application publishes a value (ADR 0092, point 5;
//! `docs/spec/plugin-api.md`). Nothing here draws or connects: a description is plain data the
//! application draws with its own components, and a request is plain data the application decides
//! about, so a Plugin is never handed a toolkit element or a socket.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use mlua::{Lua, Table, Value};

use crate::data::Data;

/// How many descriptions one Plugin may have added at once. ODK's own, provisional.
pub const MAX_CONTRIBUTIONS: usize = 64;
/// How many bytes of text a description may hold, keys included. ODK's own, provisional.
pub const MAX_DESCRIPTION_BYTES: usize = 16 * 1024;
/// How many requests one Plugin may have waiting for an answer. ODK's own, provisional.
pub const MAX_PENDING_REQUESTS: usize = 8;
/// The longest URL a request may name. ODK's own.
pub const MAX_URL_BYTES: usize = 2048;
/// How many headers a request may carry. ODK's own.
pub const MAX_HEADERS: usize = 32;
/// The most bytes a request may carry as its body. ODK's own, provisional.
pub const MAX_BODY_BYTES: usize = 64 * 1024;
/// How many names one Plugin may subscribe to. ODK's own.
pub const MAX_SUBSCRIPTIONS: usize = 32;

const METHODS: &[&str] = &["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE"];

/// A description a Plugin added to an extension point, as the application reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Contribution {
    /// The Plugin that added it.
    pub plugin: String,
    /// Its identifier: what `Host::activate` and `ui.remove` take.
    pub id: u64,
    /// The extension point it was added to.
    pub point: String,
    /// What the Plugin described, plain data the application draws with its own components.
    pub description: Data,
}

/// A request a Plugin made, as the application's handler hears it. The handler decides whether
/// to make it, and answers with `Host::answer`, now or later; it must not block and must not call
/// into the host from inside the handler.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkRequest {
    /// The Plugin that asked.
    pub plugin: String,
    /// The request's identifier, which `Host::answer` takes.
    pub id: u64,
    /// `url`, `method`, `headers` (a map of text) and `body`, checked by the host.
    pub request: Data,
}

pub(crate) type Handler = Rc<dyn Fn(NetworkRequest)>;

/// What the host and every Plugin's functions share. A Plugin's own state holds none of it: the
/// functions it is given hold a handle and look things up by its id.
#[derive(Default)]
pub(crate) struct Shared {
    points: RefCell<BTreeSet<String>>,
    contributions: RefCell<Vec<Contribution>>,
    pending: RefCell<BTreeSet<(String, u64)>>,
    subscriptions: RefCell<BTreeMap<String, BTreeMap<String, String>>>,
    handler: RefCell<Option<Handler>>,
    next: Cell<u64>,
}

impl Shared {
    pub(crate) fn register_point(&self, name: String) {
        self.points.borrow_mut().insert(name);
    }

    pub(crate) fn set_handler(&self, handler: Handler) {
        *self.handler.borrow_mut() = Some(handler);
    }

    pub(crate) fn contributions(&self, point: &str) -> Vec<Contribution> {
        self.contributions
            .borrow()
            .iter()
            .filter(|c| c.point == point)
            .cloned()
            .collect()
    }

    pub(crate) fn contribution(&self, plugin: &str, id: u64) -> Option<Contribution> {
        self.contributions
            .borrow()
            .iter()
            .find(|c| c.plugin == plugin && c.id == id)
            .cloned()
    }

    /// Takes a waiting request off the list, and says whether it was there.
    pub(crate) fn take_pending(&self, plugin: &str, id: u64) -> bool {
        self.pending.borrow_mut().remove(&(plugin.to_owned(), id))
    }

    /// The Plugins that subscribed to `name`, each with the function to call.
    pub(crate) fn subscribers(&self, name: &str) -> Vec<(String, String)> {
        self.subscriptions
            .borrow()
            .get(name)
            .map(|m| m.iter().map(|(p, f)| (p.clone(), f.clone())).collect())
            .unwrap_or_default()
    }

    /// Everything a Plugin held, for when it is unloaded: its descriptions, its waiting requests
    /// and its subscriptions go with it.
    pub(crate) fn forget(&self, plugin: &str) {
        self.contributions
            .borrow_mut()
            .retain(|c| c.plugin != plugin);
        self.pending.borrow_mut().retain(|(p, _)| p != plugin);
        for subscribers in self.subscriptions.borrow_mut().values_mut() {
            subscribers.remove(plugin);
        }
    }

    fn fresh(&self) -> u64 {
        let id = self.next.get() + 1;
        self.next.set(id);
        id
    }
}

fn refuse(what: &str, why: impl std::fmt::Display) -> mlua::Error {
    mlua::Error::runtime(format!("{what}: {why}"))
}

/// The bytes of text in a description: every string and every key.
fn text_bytes(data: &Data) -> usize {
    match data {
        Data::Text(s) => s.len(),
        Data::List(items) => items.iter().map(text_bytes).sum(),
        Data::Map(entries) => entries.iter().map(|(k, v)| k.len() + text_bytes(v)).sum(),
        _ => 0,
    }
}

/// `ui.add(point, description)` and `ui.remove(id)`.
pub(crate) fn install_ui(
    lua: &Lua,
    table: &Table,
    plugin: String,
    shared: Rc<Shared>,
) -> mlua::Result<()> {
    let (s, id) = (shared.clone(), plugin.clone());
    table.set(
        "add",
        lua.create_function(move |_, (point, description): (String, Value)| {
            if !s.points.borrow().contains(&point) {
                return Err(refuse(
                    "ui.add",
                    format!("the application has no extension point {point:?}"),
                ));
            }
            let description = Data::from_lua(&description).map_err(|e| refuse("ui.add", e))?;
            if !matches!(description, Data::Map(_)) {
                return Err(refuse("ui.add", "a description is a table with string keys"));
            }
            let bytes = text_bytes(&description);
            if bytes > MAX_DESCRIPTION_BYTES {
                return Err(refuse(
                    "ui.add",
                    format!(
                        "a description may hold {MAX_DESCRIPTION_BYTES} bytes of text, and this holds {bytes}"
                    ),
                ));
            }
            let mut all = s.contributions.borrow_mut();
            if all.iter().filter(|c| c.plugin == id).count() >= MAX_CONTRIBUTIONS {
                return Err(refuse(
                    "ui.add",
                    format!("a Plugin may have {MAX_CONTRIBUTIONS} descriptions added"),
                ));
            }
            let number = s.fresh();
            all.push(Contribution {
                plugin: id.clone(),
                id: number,
                point,
                description,
            });
            Ok(number)
        })?,
    )?;

    let id = plugin;
    table.set(
        "remove",
        lua.create_function(move |_, number: u64| {
            let mut all = shared.contributions.borrow_mut();
            let before = all.len();
            all.retain(|c| !(c.plugin == id && c.id == number));
            Ok(all.len() != before)
        })?,
    )
}

/// The request a Plugin described, checked: the URL, the method, the headers and the body, as
/// plain data in one shape.
fn check_request(value: &Value) -> Result<Data, String> {
    let Data::Map(request) = Data::from_lua(value)? else {
        return Err("a request is a table with string keys".to_owned());
    };
    for key in request.keys() {
        if !["url", "method", "headers", "body"].contains(&key.as_str()) {
            return Err(format!("there is no request field {key:?}"));
        }
    }
    let Some(Data::Text(url)) = request.get("url") else {
        return Err("a request needs a `url` of text".to_owned());
    };
    if url.len() > MAX_URL_BYTES {
        return Err(format!("a URL may be {MAX_URL_BYTES} bytes"));
    }
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("a URL starts with https:// or http://".to_owned());
    }
    let method = match request.get("method") {
        None | Some(Data::Nil) => "GET".to_owned(),
        Some(Data::Text(m)) => m.to_ascii_uppercase(),
        Some(_) => return Err("`method` is text".to_owned()),
    };
    if !METHODS.contains(&method.as_str()) {
        return Err(format!("the method is one of {}", METHODS.join(", ")));
    }
    let headers = match request.get("headers") {
        None | Some(Data::Nil) => BTreeMap::new(),
        Some(Data::Map(h)) => {
            if h.len() > MAX_HEADERS {
                return Err(format!("a request may carry {MAX_HEADERS} headers"));
            }
            for (name, value) in h {
                let Data::Text(value) = value else {
                    return Err(format!("the header {name:?} is text"));
                };
                if name.is_empty()
                    || !name.bytes().all(|b| b.is_ascii_graphic() && b != b':')
                    || value.bytes().any(|b| b == b'\r' || b == b'\n' || b == 0)
                {
                    return Err(format!("the header {name:?} is not a valid header"));
                }
            }
            h.clone()
        }
        Some(_) => return Err("`headers` is a table of text".to_owned()),
    };
    let body = match request.get("body") {
        None | Some(Data::Nil) => String::new(),
        Some(Data::Text(b)) => {
            if b.len() > MAX_BODY_BYTES {
                return Err(format!("a body may be {MAX_BODY_BYTES} bytes"));
            }
            b.clone()
        }
        Some(_) => return Err("`body` is text".to_owned()),
    };
    Ok(Data::Map(BTreeMap::from([
        ("url".to_owned(), Data::Text(url.clone())),
        ("method".to_owned(), Data::Text(method)),
        ("headers".to_owned(), Data::Map(headers)),
        ("body".to_owned(), Data::Text(body)),
    ])))
}

/// `network.request(request)` and `network.cancel(id)`.
pub(crate) fn install_network(
    lua: &Lua,
    table: &Table,
    plugin: String,
    shared: Rc<Shared>,
) -> mlua::Result<()> {
    let (s, id) = (shared.clone(), plugin.clone());
    table.set(
        "request",
        lua.create_function(move |_, request: Value| {
            let handler =
                s.handler.borrow().clone().ok_or_else(|| {
                    refuse("network.request", "the application makes no requests")
                })?;
            let request = check_request(&request).map_err(|e| refuse("network.request", e))?;
            if s.pending.borrow().iter().filter(|(p, _)| *p == id).count() >= MAX_PENDING_REQUESTS {
                return Err(refuse(
                    "network.request",
                    format!("a Plugin may have {MAX_PENDING_REQUESTS} requests waiting"),
                ));
            }
            let number = s.fresh();
            s.pending.borrow_mut().insert((id.clone(), number));
            handler(NetworkRequest {
                plugin: id.clone(),
                id: number,
                request,
            });
            Ok(number)
        })?,
    )?;

    let id = plugin;
    table.set(
        "cancel",
        lua.create_function(move |_, number: u64| Ok(shared.take_pending(&id, number)))?,
    )
}

/// `app.subscribe(name, function_name)`: the host calls the Plugin's global `function_name` with
/// the name and the value each time the application publishes `name`.
pub(crate) fn install_subscribe(
    lua: &Lua,
    table: &Table,
    plugin: String,
    shared: Rc<Shared>,
) -> mlua::Result<()> {
    table.set(
        "subscribe",
        lua.create_function(move |_, (name, function): (String, String)| {
            let mut all = shared.subscriptions.borrow_mut();
            let mine = all.values().filter(|m| m.contains_key(&plugin)).count();
            let already = all.get(&name).is_some_and(|m| m.contains_key(&plugin));
            if !already && mine >= MAX_SUBSCRIPTIONS {
                return Err(refuse(
                    "app.subscribe",
                    format!("a Plugin may subscribe to {MAX_SUBSCRIPTIONS} names"),
                ));
            }
            all.entry(name)
                .or_default()
                .insert(plugin.clone(), function);
            Ok(())
        })?,
    )
}

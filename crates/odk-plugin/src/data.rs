//! What crosses between the host and a Plugin (ADR 0092): `nil`, booleans, numbers, strings and
//! tables of them, copied. No host object is ever handed to Luau.

use std::collections::BTreeMap;

use mlua::{Lua, Value};

/// How deep a table may nest when it crosses. ODK's own.
const MAX_DEPTH: usize = 16;
/// How many values a table, with everything in it, may hold when it crosses. ODK's own.
const MAX_VALUES: usize = 10_000;

#[derive(Debug, Clone, PartialEq)]
pub enum Data {
    Nil,
    Bool(bool),
    Number(f64),
    Text(String),
    /// A table whose keys are 1 to n.
    List(Vec<Data>),
    /// A table whose keys are all strings.
    Map(BTreeMap<String, Data>),
}

impl Data {
    pub(crate) fn to_lua(&self, lua: &Lua) -> mlua::Result<Value> {
        Ok(match self {
            Data::Nil => Value::Nil,
            Data::Bool(b) => Value::Boolean(*b),
            Data::Number(n) => Value::Number(*n),
            Data::Text(s) => Value::String(lua.create_string(s)?),
            Data::List(items) => {
                let table = lua.create_table()?;
                for (i, item) in items.iter().enumerate() {
                    table.raw_set(i + 1, item.to_lua(lua)?)?;
                }
                Value::Table(table)
            }
            Data::Map(entries) => {
                let table = lua.create_table()?;
                for (key, item) in entries {
                    table.raw_set(key.as_str(), item.to_lua(lua)?)?;
                }
                Value::Table(table)
            }
        })
    }

    /// The plain data a Lua value is, or why it is not: a function, a thread, userdata, a table
    /// with mixed keys, one that is too deep or too big, or text that is not UTF-8.
    pub(crate) fn from_lua(value: &Value) -> Result<Data, String> {
        let mut budget = MAX_VALUES;
        Self::read(value, 0, &mut budget)
    }

    fn read(value: &Value, depth: usize, budget: &mut usize) -> Result<Data, String> {
        *budget = budget
            .checked_sub(1)
            .ok_or_else(|| format!("more than {MAX_VALUES} values"))?;
        Ok(match value {
            Value::Nil => Data::Nil,
            Value::Boolean(b) => Data::Bool(*b),
            Value::Integer(i) => Data::Number(*i as f64),
            Value::Number(n) => Data::Number(*n),
            Value::String(s) => Data::Text(
                s.to_str()
                    .map_err(|_| "text that is not UTF-8".to_owned())?
                    .to_string(),
            ),
            Value::Table(table) => {
                if depth >= MAX_DEPTH {
                    return Err(format!("tables nested more than {MAX_DEPTH} deep"));
                }
                let mut pairs = Vec::new();
                for pair in table.clone().pairs::<Value, Value>() {
                    pairs.push(pair.map_err(|e| e.to_string())?);
                }
                if pairs
                    .iter()
                    .all(|(k, _)| matches!(k, Value::Integer(_) | Value::Number(_)))
                {
                    let mut items: Vec<Option<Data>> = vec![None; pairs.len()];
                    for (k, v) in &pairs {
                        let index = match k {
                            Value::Integer(i) => *i,
                            Value::Number(n) if n.fract() == 0.0 => *n as i64,
                            _ => return Err("a table with keys that are not 1 to n".to_owned()),
                        };
                        let slot = usize::try_from(index - 1)
                            .ok()
                            .filter(|i| *i < items.len())
                            .ok_or_else(|| "a table with keys that are not 1 to n".to_owned())?;
                        items[slot] = Some(Self::read(v, depth + 1, budget)?);
                    }
                    Data::List(
                        items
                            .into_iter()
                            .collect::<Option<Vec<_>>>()
                            .ok_or_else(|| "a table with keys that are not 1 to n".to_owned())?,
                    )
                } else if pairs.iter().all(|(k, _)| matches!(k, Value::String(_))) {
                    let mut entries = BTreeMap::new();
                    for (k, v) in &pairs {
                        let Value::String(key) = k else {
                            unreachable!()
                        };
                        let key = key
                            .to_str()
                            .map_err(|_| "a key that is not UTF-8".to_owned())?
                            .to_string();
                        entries.insert(key, Self::read(v, depth + 1, budget)?);
                    }
                    Data::Map(entries)
                } else {
                    return Err("a table that mixes string keys with others".to_owned());
                }
            }
            other => {
                return Err(format!(
                    "a {}, and only nil, booleans, numbers, strings and tables of them cross",
                    other.type_name()
                ));
            }
        })
    }
}

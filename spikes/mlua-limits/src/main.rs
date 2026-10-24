//! Spike: do mlua's memory limit and interrupt work with Luau and the sandbox on?
//! (docs/checks.md)

use mlua::{Error, Lua, VmState};
use std::time::{Duration, Instant};

fn main() -> mlua::Result<()> {
    // 1. Memory limit.
    let lua = Lua::new();
    lua.sandbox(true)?;
    println!("memory used at start: {} bytes", lua.used_memory());
    lua.set_memory_limit(8 * 1024 * 1024)?;
    let r = lua
        .load("local t = {} for i = 1, 10000000 do t[i] = {i} end return #t")
        .eval::<i64>();
    match &r {
        Err(Error::MemoryError(m)) => println!("memory limit: stopped with MemoryError ({m})"),
        other => println!("memory limit: UNEXPECTED {other:?}"),
    }
    println!("memory used after: {} bytes", lua.used_memory());
    // The VM must still be usable after hitting the limit.
    lua.set_memory_limit(0)?;
    println!(
        "usable after limit: {:?}",
        lua.load("return 1 + 1").eval::<i64>()
    );

    // 2. Interrupt on a deadline, against an endless loop.
    let lua = Lua::new();
    lua.sandbox(true)?;
    let deadline = Instant::now() + Duration::from_millis(200);
    lua.set_interrupt(move |_| {
        if Instant::now() > deadline {
            Err(Error::runtime("plugin ran too long"))
        } else {
            Ok(VmState::Continue)
        }
    });
    let start = Instant::now();
    let r = lua.load("while true do end").exec();
    println!(
        "interrupt: {:?} after {:?}",
        r.map_err(|e| e.to_string()),
        start.elapsed()
    );

    // 3. What the sandbox leaves visible.
    let lua = Lua::new();
    lua.sandbox(true)?;
    for name in [
        "io",
        "os",
        "debug",
        "package",
        "require",
        "loadstring",
        "dofile",
        "getfenv",
    ] {
        let v: mlua::Value = lua.globals().get(name)?;
        println!(
            "global {name}: {}",
            if v.is_nil() { "absent" } else { "PRESENT" }
        );
    }
    let os_execute = lua.load("return os and os.execute").eval::<mlua::Value>();
    println!("os.execute: {:?}", os_execute.map(|v| v.type_name()));
    // Sandbox isolates writes to globals between scripts?
    lua.load("leak = 1").exec()?;
    println!(
        "global write persists across chunks: {:?}",
        lua.load("return leak").eval::<Option<i64>>()
    );
    for lib in ["os", "debug"] {
        let keys: String = lua
            .load(format!("local k = {{}} for n in pairs({lib}) do k[#k+1] = n end table.sort(k) return table.concat(k, ' ')"))
            .eval()?;
        println!("{lib} contains: {keys}");
    }
    println!(
        "require('x'): {:?}",
        lua.load("return require('x')")
            .eval::<mlua::Value>()
            .map(|v| v.type_name())
            .map_err(|e| e.to_string().lines().next().unwrap_or("").to_string())
    );
    // Per-chunk environments: does a plugin-private environment keep writes apart?
    let env_a = lua.create_table()?;
    lua.load("secret = 42")
        .set_environment(env_a.clone())
        .exec()?;
    println!(
        "write inside own environment leaks to globals: {:?}",
        lua.load("return secret").eval::<Option<i64>>()
    );
    Ok(())
}

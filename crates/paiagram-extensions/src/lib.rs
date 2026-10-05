// SPDX-License-Identifier: MPL-2.0
#![doc = include_str!("../README.md")]

use std::sync::Arc;

use paiagram_core::WorldSnapshot;
use parking_lot::Mutex;
use rayon::spawn;
use rquickjs::class::Trace;
use rquickjs::{CatchResultExt, Class, Context, Ctx, Function, JsLifetime, Runtime};

#[derive(Default, Debug)]
pub enum ScriptRunningStatus {
    Running,
    Finished(Result<WorldSnapshot, String>),
    #[default]
    Idle,
}

pub fn run_script(
    snap: WorldSnapshot,
    status: Arc<Mutex<ScriptRunningStatus>>,
    script_str: Arc<str>,
) {
    *status.lock() = ScriptRunningStatus::Running;
    spawn(move || {
        let snap = run_script_inner(snap, &script_str);
        *status.lock() = ScriptRunningStatus::Finished(snap);
    });
}

#[derive(Trace, JsLifetime)]
#[rquickjs::class(rename_all = "camelCase")]
struct JsWorldSnapshot {
    #[qjs(skip_trace)]
    inner: WorldSnapshot,
}

#[rquickjs::methods]
impl JsWorldSnapshot {
    pub fn do_thing(&mut self) {}
}

fn run_script_inner(snap: WorldSnapshot, script_str: &str) -> Result<WorldSnapshot, String> {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();
    ctx.with(|ctx| {
        install_console(ctx.clone()).map_err(|e| e.to_string())?;
        let cls = Class::instance(ctx.clone(), JsWorldSnapshot { inner: snap })
            .map_err(|e| e.to_string())?;
        ctx.globals().set("world", cls.clone()).map_err(|e| e.to_string())?;
        ctx.eval::<(), _>(script_str).catch(&ctx).map_err(|e| e.to_string())?;
        Ok(std::mem::take(&mut cls.borrow_mut().inner))
    })
}

/// JavaScript shim for the `console` global. It stringifies the arguments in JS and forwards a
/// single already-formatted message to the native `__log(level, message)` function, so the native
/// side only deals with owned Rust types.
const CONSOLE_SHIM: &str = r#"
(() => {
    const fmt = (value) => {
        if (typeof value === "string") return value;
        if (value === null) return "null";
        if (value === undefined) return "undefined";
        if (typeof value === "object") {
            try { return JSON.stringify(value); } catch (_) { return String(value); }
        }
        return String(value);
    };
    const log = globalThis.__log;
    const make = (level) => (...args) => log(level, args.map(fmt).join(" "));
    globalThis.console = {
        log: make(1),
        info: make(1),
        debug: make(2),
        warn: make(3),
        error: make(4),
        trace: make(5),
    };
})();
"#;

/// Installs a `console` global whose methods forward to the `log` crate.
fn install_console<'js>(ctx: Ctx<'js>) -> rquickjs::Result<()> {
    let log_fn = Function::new(ctx.clone(), |level: u32, message: String| {
        let level = match level {
            2 => log::Level::Debug,
            3 => log::Level::Warn,
            4 => log::Level::Error,
            5 => log::Level::Trace,
            _ => log::Level::Info,
        };
        log::log!(target: "script", level, "{message}");
    })?;
    ctx.globals().set("__log", log_fn)?;
    ctx.eval::<(), _>(CONSOLE_SHIM)?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn get_world_and_return() {
        env_logger::init();
        let script_str = r#"
world()
"#;
        let status: Arc<Mutex<ScriptRunningStatus>> = Default::default();
        run_script(WorldSnapshot::default(), status.clone(), script_str.into());
        let a = status.lock();
        dbg!(&a);
    }

    #[test]
    fn console_forwards_to_log() {
        env_logger::init();
        let result = run_script_inner(
            WorldSnapshot::default(),
            r#"
console.log("hello", 42, { a: 1 }, [1, 2], null, undefined, true);
console.info("info");
console.debug("debug");
console.warn("warn");
console.error("error");
console.trace("trace");
"#,
        );
        assert!(result.is_ok(), "console.* raised: {:?}", result.err());
    }
}

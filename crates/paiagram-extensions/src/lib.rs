// SPDX-License-Identifier: MPL-2.0
#![doc = include_str!("../README.md")]

use std::sync::{Arc, LazyLock};
pub mod config_ui;

use config_ui::{ConfigUi, ReturnValueMap};
use paiagram_core::World;
use parking_lot::Mutex;
use rayon::spawn;
use rquickjs::{CatchResultExt, Class, Context, Ctx, Function, Module, Object, Runtime, Value};

mod runtime;
pub use runtime::{JsDataExporter, JsIo, JsRuntimeData};

#[derive(Default, Debug)]
pub enum RunningStatus<T> {
    Running,
    Finished(rquickjs::Result<T>),
    #[default]
    Idle,
}

pub type ConfigRunningStatus = RunningStatus<Vec<ConfigUi>>;
pub type ScriptRunningStatus = RunningStatus<Option<World>>;

static RQUICKJS_RUNTIME: LazyLock<Runtime> = LazyLock::new(|| Runtime::new().unwrap());

/// Evaluate the config
pub fn eval_config(
    runtime_data: JsRuntimeData,
    status: Arc<Mutex<ConfigRunningStatus>>,
    script_str: Arc<str>,
) {
    *status.lock() = ConfigRunningStatus::Running;
    spawn(move || {
        let result = with_context(
            runtime_data,
            None,
            &script_str,
            |ctx, module, world, _io| {
                let entry: Function = module.get("config_ui")?;
                let value: Value = entry.call((world,))?;
                ConfigUi::parse_many(ctx, value)
            },
        );
        *status.lock() = ConfigRunningStatus::Finished(result);
    });
}

/// Execute the extension's `run` entrypoint
pub fn run_script(
    runtime_data: JsRuntimeData,
    io: JsIo,
    return_values: ReturnValueMap,
    status: Arc<Mutex<ScriptRunningStatus>>,
    script_str: Arc<str>,
) {
    *status.lock() = ScriptRunningStatus::Running;
    spawn(move || {
        let result = with_context(
            runtime_data,
            Some(io),
            &script_str,
            |_ctx, module, world, io| {
                let entry: Function = module.get("run")?;
                let io = io.expect("run always receives io");
                entry.call::<_, ()>((world.clone(), return_values, io))?;
                Ok(Some(std::mem::take(&mut world.borrow_mut().world)))
            },
        );
        *status.lock() = ScriptRunningStatus::Finished(result);
    });
}

fn with_context<T>(
    runtime_data: JsRuntimeData,
    io: Option<JsIo>,
    script_str: &str,
    f: impl for<'js> FnOnce(
        &Ctx<'js>,
        Module<'js, rquickjs::module::Evaluated>,
        Class<'js, JsRuntimeData>,
        Option<Class<'js, JsIo>>,
    ) -> rquickjs::Result<T>,
) -> rquickjs::Result<T> {
    let ctx = Context::full(&RQUICKJS_RUNTIME).unwrap();
    ctx.with(|ctx| {
        install_console(&ctx)?;
        let world = Class::instance(ctx.clone(), runtime_data)?;
        let io = io.map(|io| Class::instance(ctx.clone(), io)).transpose()?;

        let script_error = |err: rquickjs::CaughtError<'_>| {
            rquickjs::Error::new_from_js_message("script", "extension", err.to_string())
        };
        let module = Module::declare(ctx.clone(), "extension.js", script_str)
            .catch(&ctx)
            .map_err(script_error)?;
        let (module, promise) = module.eval().catch(&ctx).map_err(script_error)?;
        promise.finish::<()>().catch(&ctx).map_err(script_error)?;

        f(&ctx, module, world, io).catch(&ctx).map_err(|err| {
            rquickjs::Error::new_from_js_message("entry point", "extension", err.to_string())
        })
    })
}

/// JavaScript factory for the global `console` object. It takes the Rust log bridge as a
/// parameter, so no extra globals are needed.
const CONSOLE_SHIM: &str = r#"
((log) => {
    const fmt = (value) => {
        if (typeof value === "string") return value;
        if (value === null) return "null";
        if (value === undefined) return "undefined";
        if (typeof value === "object") {
            try { return JSON.stringify(value); } catch (_) { return String(value); }
        }
        return String(value);
    };
    const make = (level) => (...args) => log(level, args.map(fmt).join(" "));
    return {
        log: make(1),
        info: make(1),
        debug: make(2),
        warn: make(3),
        error: make(4),
        trace: make(5),
    };
})
"#;

/// Installs the standard `console` global, whose methods forward to the `log` crate.
fn install_console<'js>(ctx: &Ctx<'js>) -> rquickjs::Result<()> {
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
    let factory: Function = ctx.eval(CONSOLE_SHIM)?;
    let console: Object = factory.call((log_fn,))?;
    ctx.globals().set("console", console)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use paiagram_core::{Key, TripKey};
    use paiagram_selection::SelectedItem;
    use rquickjs::{Array, BigInt, Context, Runtime, Value};

    use super::*;

    #[test]
    fn selected_items_serialize_keys_exactly() {
        // Allocate a key with nonzero counter bits so f64 precision loss would be visible.
        let _ = TripKey::new();
        let trip = TripKey::new();
        let data = JsRuntimeData {
            world: World::default(),
            selected_items: SelectedItem::Trip(trip.into()).into(),
        };

        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            let value: Value = data.selected_items(ctx.clone()).unwrap();
            let object = value.into_object().unwrap();
            let trips: Array = object.get("Trips").unwrap();
            let first: BigInt = trips.get(0).unwrap();
            assert_eq!(first.to_i64().unwrap() as u64, trip.to_bits());
        });
    }
}

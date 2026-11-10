// SPDX-License-Identifier: MPL-2.0
#![doc = include_str!("../README.md")]

use std::sync::{Arc, LazyLock, mpsc};
pub mod config_ui;

use config_ui::{ConfigUi, ReturnValueMap};
use paiagram_core::World;
use parking_lot::Mutex;
use rquickjs::{
    CatchResultExt, Class, Context, Ctx, Function, Module, Object, Runtime, Value, WriteOptions,
};
pub use rquickjs::{Error, Result};
use serde::Deserialize;

mod runtime_data;
pub use runtime_data::{JsDataExporter, JsIo, JsRuntimeData};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub author: Option<String>,
}

impl Metadata {
    pub fn parse<'js>(_ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        rquickjs_serde::from_value(value).map_err(|err| {
            Error::new_from_js_message("rquickjs-serde", "Metadata", err.to_string())
        })
    }
}

#[derive(Clone, Debug)]
pub struct ByteCodes {
    inner: Arc<[u8]>,
    pub metadata: Metadata,
}

impl ByteCodes {
    pub fn compile(script_str: &str) -> Result<Self> {
        let ctx = Context::full(&RQUICKJS_RUNTIME).unwrap();
        ctx.with(|ctx| {
            install_console(&ctx)?;
            let script_error = |err: rquickjs::CaughtError<'_>| {
                Error::new_from_js_message("script", "extension", err.to_string())
            };
            let module = Module::declare(ctx.clone(), "extension.js", script_str)
                .catch(&ctx)
                .map_err(script_error)?;
            let bytes = module.write(WriteOptions::default()).catch(&ctx).map_err(script_error)?;
            let (module, promise) = module.eval().catch(&ctx).map_err(script_error)?;
            promise.finish::<()>().catch(&ctx).map_err(script_error)?;
            let metadata_val: Value = module.get("metadata").catch(&ctx).map_err(script_error)?;
            let metadata = Metadata::parse(&ctx, metadata_val)?;
            Ok(Self { inner: bytes.into(), metadata })
        })
    }
}

/// Compile the script into bytecodes and extract its metadata
pub fn compile_script(script_str: Arc<str>, sender: mpsc::Sender<Result<ByteCodes>>) {
    rayon::spawn(move || {
        let _ = sender.send(ByteCodes::compile(&script_str));
    });
}

#[derive(Default, Debug)]
pub enum RunningStatus<T> {
    Running,
    Finished(Result<T>),
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
    bytecodes: ByteCodes,
) {
    *status.lock() = ConfigRunningStatus::Running;
    rayon::spawn(move || {
        let result = with_context(runtime_data, None, &bytecodes, |ctx, module, world, _io| {
            let entry: Function = module.get("config_ui")?;
            let value: Value = entry.call((world,))?;
            ConfigUi::parse_many(ctx, value)
        });
        *status.lock() = ConfigRunningStatus::Finished(result);
    });
}

/// Execute the extension's `run` entrypoint
pub fn run_script(
    runtime_data: JsRuntimeData,
    io: JsIo,
    return_values: ReturnValueMap,
    status: Arc<Mutex<ScriptRunningStatus>>,
    bytecodes: ByteCodes,
) {
    *status.lock() = ScriptRunningStatus::Running;
    rayon::spawn(move || {
        let result = with_context(runtime_data, Some(io), &bytecodes, |_ctx, module, world, io| {
            let entry: Function = module.get("run")?;
            let io = io.expect("run always receives io");
            entry.call::<_, ()>((world.clone(), return_values, io))?;
            Ok(Some(std::mem::take(&mut world.borrow_mut().world)))
        });
        *status.lock() = ScriptRunningStatus::Finished(result);
    });
}

fn with_context<T>(
    runtime_data: JsRuntimeData,
    io: Option<JsIo>,
    bytecodes: &ByteCodes,
    f: impl for<'js> FnOnce(
        &Ctx<'js>,
        Module<'js, rquickjs::module::Evaluated>,
        Class<'js, JsRuntimeData>,
        Option<Class<'js, JsIo>>,
    ) -> Result<T>,
) -> Result<T> {
    let ctx = Context::full(&RQUICKJS_RUNTIME).unwrap();
    ctx.with(|ctx| {
        install_console(&ctx)?;
        let world = Class::instance(ctx.clone(), runtime_data)?;
        let io = io.map(|io| Class::instance(ctx.clone(), io)).transpose()?;

        let script_error = |err: rquickjs::CaughtError<'_>| {
            Error::new_from_js_message("script", "extension", err.to_string())
        };
        // SAFETY: `bytecodes.inner` was produced by `Module::write` on a valid declared module.
        let module = unsafe { Module::load(ctx.clone(), &bytecodes.inner) }
            .catch(&ctx)
            .map_err(script_error)?;
        let (module, promise) = module.eval().catch(&ctx).map_err(script_error)?;
        promise.finish::<()>().catch(&ctx).map_err(script_error)?;

        f(&ctx, module, world, io)
            .catch(&ctx)
            .map_err(|err| Error::new_from_js_message("entry point", "extension", err.to_string()))
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
fn install_console<'js>(ctx: &Ctx<'js>) -> Result<()> {
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

    #[test]
    fn compile_and_eval_bytecode() {
        let script = include_str!("../../../extensions/export_focused.js");
        let bytecodes = ByteCodes::compile(script).unwrap();
        assert_eq!(bytecodes.metadata.id, "paiagram.export_focused");
        let data = JsRuntimeData {
            world: World::default(),
            selected_items: paiagram_selection::SelectedItems::None,
        };
        let ui = with_context(data, None, &bytecodes, |ctx, module, world, _io| {
            let entry: Function = module.get("config_ui")?;
            let value: Value = entry.call((world,))?;
            ConfigUi::parse_many(ctx, value)
        })
        .unwrap();
        assert!(!ui.is_empty());
    }
}

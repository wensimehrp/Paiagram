use std::sync::mpsc;

use paiagram_core::route::MareyCache;
use paiagram_core::{Key, Wfc, World};
use paiagram_selection::SelectedItems;
use rquickjs::class::Trace;
use rquickjs::{Ctx, JsLifetime, Object, Value};
use serde::{Deserialize, Serialize};

/// The side-effecting half of the extension API.
#[derive(Trace, JsLifetime)]
#[rquickjs::class(rename_all = "camelCase")]
pub struct JsIo {
    /// wasm only allows downloading files in the main thread, hence the file sender to send data
    /// to the main thread.
    #[qjs(skip_trace)]
    pub file_sender: mpsc::Sender<JsDataExporter>,
}

#[rquickjs::methods(rename_all = "camelCase")]
impl JsIo {
    /// Request a file download from JS.
    pub fn save_file(&mut self, filename: String, content: String) {
        let info = JsDataExporter { filename, content };
        self.file_sender.send(info).unwrap();
    }
}

pub struct JsDataExporter {
    pub filename: String,
    pub content: String,
}

impl paiagram_rw::ExportObject for JsDataExporter {
    fn extension(&self) -> &str {
        std::path::Path::new(&self.filename)
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("txt")
    }
    fn default_filename(&self) -> &str {
        std::path::Path::new(&self.filename)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(&self.filename)
    }
    fn write_to_vec(&mut self, mut buf: Vec<u8>) -> std::io::Result<Vec<u8>> {
        buf.extend_from_slice(self.content.as_bytes());
        Ok(buf)
    }
}

#[derive(Serialize, Deserialize)]
struct JsWorldFieldContainer<D, C> {
    data: D,
    cache: C,
}

type JsWfc<D, C> = JsWorldFieldContainer<D, C>;

impl<D, C> From<Wfc<D, C>> for JsWfc<D, C> {
    fn from(Wfc { data, cache }: Wfc<D, C>) -> Self {
        Self { data, cache }
    }
}

impl<D, C> From<JsWfc<D, C>> for Wfc<D, C> {
    fn from(JsWfc { data, cache }: JsWfc<D, C>) -> Self {
        Self { data, cache }
    }
}

/// The read-only half of the extension API, exposed to both entry points as `world`.
#[derive(Trace, JsLifetime)]
#[rquickjs::class(rename_all = "camelCase")]
pub struct JsRuntimeData {
    #[qjs(skip_trace)]
    pub world: World,
    #[qjs(skip_trace)]
    pub selected_items: SelectedItems,
}

type Ret<'a> = rquickjs::Result<Value<'a>>;

fn format_serde_err<F>(_: F) -> impl FnOnce(rquickjs_serde::Error) -> rquickjs::Error {
    |err| {
        rquickjs::Error::new_from_js_message(
            std::any::type_name::<F>(),
            "rquickjs-serde",
            err.to_string(),
        )
    }
}

#[rquickjs::methods(rename_all = "camelCase")]
impl JsRuntimeData {
    /// Returns the selected items.
    pub fn selected_items<'js>(&self, ctx: Ctx<'js>) -> Ret<'js> {
        rquickjs_serde::to_value(ctx, &self.selected_items)
            .map_err(format_serde_err(Self::selected_items))
    }

    /// Returns every route key in the world.
    pub fn route_info<'js>(&self, ctx: Ctx<'js>) -> Ret<'js> {
        let keys: Vec<_> = self.world.routes.keys().copied().collect();
        rquickjs_serde::to_value(ctx, keys).map_err(format_serde_err(Self::route_info))
    }

    /// Returns a plain object mapping each trip key (as a decimal string) to its Marey lines.
    pub fn marey_cache<'js>(&self, ctx: Ctx<'js>) -> Ret<'js> {
        let caches = Object::new(ctx.clone())?;
        for route in self.world.routes.values() {
            let mut cache = MareyCache::default();
            route.intervals.populate_trips(&self.world, &mut cache);
            for (trip_key, polylines) in &cache.map {
                let value = rquickjs_serde::to_value(ctx.clone(), polylines)
                    .map_err(format_serde_err(Self::marey_cache))?;
                caches.set(trip_key.to_bits().to_string(), value)?;
            }
        }
        Ok(caches.into_value())
    }
}

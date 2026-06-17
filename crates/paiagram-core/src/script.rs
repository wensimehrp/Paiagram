// SPDX-License-Identifier: MPL-2.0
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use ecow::{EcoString, EcoVec};
use rhai::{Dynamic, Engine, EvalAltResult, ImmutableString};

use super::{Command, TripView, WorldSnapshot};
use crate::TEntry;

#[derive(Default, Clone)]
struct ScriptWorldInner {
    world: WorldSnapshot,
    command_stack: Vec<Command>,
}

// we use rc here since our script is not running on multiple threads
/// The reference of the world owned by the script. The script may create multiple worlds when
/// running.
#[derive(Clone)]
struct ScriptWorld(Rc<RefCell<ScriptWorldInner>>);

impl ScriptWorld {
    fn new() -> Self {
        Self(Rc::new(RefCell::new(ScriptWorldInner::default())))
    }
    fn apply_command(&self, cmd: Command) {
        let mut world = self.0.borrow_mut();
        world.command_stack.push(cmd.clone());
        world.world.apply_command(cmd);
    }
}

macro_rules! generate_rhai_world_module {
    (
        keys { $(
            type $key_ident:ident = $target_path:path;
        )*}
        commands { $(
            $(#[$fn_attr:meta])*
            $cmd_variant:ident( $($arg:ident: $arg_ty:ty),* $(,)? ) {
                $($cmd_field:ident: $field_constructor:expr),*
            }
        )* }
    ) => {
        paste::paste! {
            #[rhai::plugin::export_module]
            mod rhai_module {
                type World = ScriptWorld;

                $(
                    type $key_ident = $target_path;

                    /// Returns a new key. This function returns a randomly generated
                    /// key, thus it is volatile.
                    #[rhai_fn(volatile)]
                    pub fn [<new_ $key_ident:snake>]() -> $key_ident {
                        $target_path::new()
                    }
                )*

                $(
                    $(#[$fn_attr])*
                    #[rhai_fn(return_raw)]
                    pub fn [<$cmd_variant:snake>](
                        world: &mut World,
                        $($arg: $arg_ty),*
                    ) -> Result<(), Box<EvalAltResult>> {
                        let cmd: Command = Command::$cmd_variant { $(
                            $cmd_field: $field_constructor,
                        )* };
                        // Actually this step might also fail
                        world.apply_command(cmd);
                        Ok(())
                    }
                )*

                pub fn replace_with(world: &mut World, new_world: World) {
                    let inner_ref = new_world.0.borrow();
                    let snapshot = Box::new(inner_ref.world.clone());
                    world.apply_command(Command::LoadWorld { snapshot });
                }

                /// Makes sure that all commands are covered
                #[allow(dead_code)]
                #[doc(hidden)]
                fn __check_command_enums(cmd: Command) {
                    match cmd {
                        $(
                            Command::$cmd_variant { .. } => {}
                        )*
                        Command::UnloadWorld => {}
                        Command::Macro(_) => {}
                        _ => {}
                    }
                }
            }
        }
    };
}

fn extract_class(class: rhai::Dynamic) -> Result<Option<crate::ClassKey>, Box<EvalAltResult>> {
    if class.is_unit() {
        return Ok(None);
    } else if let Some(key) = class.try_cast::<crate::ClassKey>() {
        return Ok(Some(key));
    } else {
        return Err("Class key must be UNIT or an actual key!".into());
    }
}

fn extract_entries(entries: rhai::Array) -> Result<EcoVec<TEntry>, Box<EvalAltResult>> {
    todo!()
}

generate_rhai_world_module!(
    keys {
        type TripKey = crate::TripKey;
        type VehicleKey = crate::VehicleKey;
        type StationKey = crate::StationKey;
        type IntervalKey = crate::IntervalKey;
        type ClassKey = crate::ClassKey;
        type RouteKey = crate::RouteKey;
    }
    commands {
        AddTrip(key: TripKey, name: ImmutableString, class: Dynamic) {
            key: key,
            view: TripView {
                name: EcoString::from(name.as_str()),
                entries: EcoVec::new(), // TODO: fix this
                class: extract_class(class)?,
            }
        }
        RenameTrip(key: TripKey, name: ImmutableString) {
            key: key,
            name: EcoString::from(name.as_str())
        }
        ChangeTripEntries(key: TripKey, entries: rhai::Array) {
            key: key,
            entries: extract_entries(entries)?
        }
        ChangeTripClass(key: TripKey, class: Dynamic) {
            key: key,
            class: extract_class(class)?
        }
    }
);

/// Executes the rhai script.
/// Rhai scripts are a quick way to define user macros, fetch info from online sources, and automate
/// some tasks.
///
/// In web builds, this would print to the JS console.
/// In desktop builds, this would print to `stdout`.
pub fn execute_rhai_script(
    world: WorldSnapshot,
    src: Arc<str>,
    on_print: impl Fn(&str) + 'static,
    on_debug: impl Fn(&str, Option<&str>, rhai::Position) + 'static,
    on_progress: impl Fn(u64) -> Option<rhai::Dynamic> + 'static,
) -> Result<Vec<Command>, String> {
    let master_world = ScriptWorld(Rc::new(RefCell::new(ScriptWorldInner {
        world,
        command_stack: Vec::new(),
    })));

    let mut engine = Engine::new();
    engine
        .on_print(on_print)
        .on_debug(on_debug)
        .on_progress(on_progress);
    let module = rhai::plugin::exported_module!(rhai_module);
    engine.register_global_module(module.into());

    {
        let fn_world = master_world.clone();
        engine.register_fn("get_world", move || -> ScriptWorld { fn_world.clone() });
        engine.register_fn("new_world", move || -> ScriptWorld { ScriptWorld::new() });
    }

    if let Err(e) = engine.eval::<rhai::Dynamic>(&src) {
        return Err(e.to_string());
    };

    drop(engine);

    // We don't care if async tasks still hold an Rc clone, we just take the data.
    let commands = std::mem::take(&mut master_world.0.borrow_mut().command_stack);

    Ok(commands)
}

#[cfg(test)]
mod test {
    use super::execute_rhai_script;
    use crate::WorldSnapshot;

    type E = Result<(), Box<dyn std::error::Error>>;
    #[test]
    fn test_script_exec() -> E {
        let src = include_str!("script/get_world.rhai");
        let res = execute_rhai_script(
            WorldSnapshot::default(),
            src.into(),
            |s| println!("{}", s),
            |s, _, p| println!("{:?}: {}", p, s),
            |_c| None,
        )?;
        println!("Execute script result");
        println!("{:#?}", res);
        Ok(())
    }
}

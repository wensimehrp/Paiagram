// SPDX-License-Identifier: MPL-2.0
use std::cell::RefCell;
use std::rc::Rc;

use ecow::EcoString;
#[cfg(target_arch = "wasm32")]
use gloo_worker::Spawnable;
#[cfg(target_arch = "wasm32")]
use gloo_worker::oneshot::oneshot;
use rhai::{Engine, ImmutableString};

use super::{Command, WorldSnapshot};

#[derive(Default, Clone)]
struct ScriptWorldInner {
    world: WorldSnapshot,
    command_stack: Vec<Command>,
}

// we use rc here since our script is not running on multiple threads
/// The reference of the world owned by the script. The script may create multiple worlds when running.
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
        $(type $key_ident:ident = $target_path:path;)*
    ) => {
        paste::paste! {
            #[rhai::plugin::export_module]
            mod rhai_module {
                type World = ScriptWorld;

                // The macro expands these inline
                $(
                    type $key_ident = $target_path;

                    pub fn [<new_ $key_ident:snake>]() -> $key_ident {
                        $target_path::new()
                    }
                )*

                // Your standard manual functions go here
                pub fn replace_with(world: &mut World, new_world: World) {
                    let inner_ref = new_world.0.borrow();
                    let snapshot = Box::new(inner_ref.world.clone());
                    world.apply_command(Command::LoadWorld { snapshot });
                }
            }
        }
    };
}

generate_rhai_world_module!(
    type TripKey = crate::TripKey;
    type VehicleKey = crate::VehicleKey;
    type StationKey = crate::StationKey;
    type IntervalKey = crate::IntervalKey;
    type ClassKey = crate::ClassKey;
    type RouteKey = crate::RouteKey;
);

/// Executes the rhai script.
/// Rhai scripts are a quick way to define user macros, fetch info from online sources, and automate
/// some tasks.
///
/// In web builds, this would print to the JS console.
/// In desktop builds, this would print to `stdout`.
#[cfg_attr(target_arch = "wasm32", oneshot)]
pub fn execute_rhai_script(world: WorldSnapshot, src: String) -> Result<Vec<Command>, String> {
    let master_world = ScriptWorld(Rc::new(RefCell::new(ScriptWorldInner {
        world,
        command_stack: Vec::new(),
    })));

    let mut engine = Engine::new();
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
    fn push_many_functions() -> E {
        let src = include_str!("script/get_world.rhai");
        let res = execute_rhai_script(WorldSnapshot::default(), src.to_string())?;
        println!("Execute script result");
        println!("{:?}", res);
        Ok(())
    }
}

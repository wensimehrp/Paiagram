use bevy::prelude::*;

#[derive(Debug)]
pub enum AutosaveError {
    SerializationError,
}

pub fn autosave(app: &mut App, storage: &mut dyn eframe::Storage) -> Result<(), AutosaveError> {
    info!("Saving state...");
    let world = app.world_mut();
    let time = world.remove_resource::<Time<Real>>();
    let scene = DynamicSceneBuilder::from_world(&world)
        .extract_entities(
            // we do this instead of a query, in order to completely sidestep default query filters.
            // while we could use `Allow<_>`, this wouldn't account for custom disabled components
            world
                .archetypes()
                .iter()
                .flat_map(bevy::ecs::archetype::Archetype::entities)
                .map(bevy::ecs::archetype::ArchetypeEntity::id),
        )
        .extract_resources()
        .build();
    if let Some(time) = time {
        world.insert_resource(time);
    }
    let type_registry = world.resource::<AppTypeRegistry>().read();
    match scene.serialize(&type_registry) {
        Ok(serialized) => {
            eframe::set_value(storage, "paiagram_state", &serialized);
            Ok(())
        }
        Err(e) => {
            error!("Failed to serialize state: {}", e);
            Err(AutosaveError::SerializationError)
        }
    }
}

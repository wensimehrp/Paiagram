pub mod diagram;
pub mod station_timetable;
pub mod tree_view;
pub mod vehicle;

use bevy::ecs::entity::Entity;

/// The page cache. Lots of r/w, few insertions, good locality, fast executions.
#[derive(Default, Debug)]
pub struct PageCache<T> {
    keys: Vec<Entity>,
    vals: Vec<T>,
}

const LINEAR_THRESHOLD: usize = 64;

impl<T> PageCache<T> {
    /// Get the page cache or insert with a custom value.
    pub fn get_mut_or_insert_with<F>(&mut self, query_key: Entity, make_val: F) -> &mut T
    where
        F: FnOnce() -> T,
    {
        if self.keys.len() < LINEAR_THRESHOLD
            && let Some(idx) = self.keys.iter().position(|e| *e == query_key)
        {
            return &mut self.vals[idx];
        }
        return match self.keys.binary_search(&query_key) {
            Ok(idx) => &mut self.vals[idx],
            Err(idx) => {
                self.keys.insert(idx, query_key);
                self.vals.insert(idx, make_val());
                return &mut self.vals[idx];
            }
        };
    }
}

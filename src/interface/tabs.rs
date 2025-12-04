pub mod diagram;
pub mod station_timetable;
pub mod tree_view;
pub mod vehicle;
pub mod displayed_lines;
pub mod classes;
pub mod start;

/// The page cache. Lots of r/w, few insertions, good locality, fast executions.
#[derive(Debug)]
pub struct PageCache<K, V>
where
    K: PartialEq + Ord,
{
    keys: Vec<K>,
    vals: Vec<V>,
}

const LINEAR_THRESHOLD: usize = 64;

impl<K, V> PageCache<K, V>
where
    K: PartialEq + Ord,
{
    /// Get the page cache or insert with a custom value.
    pub fn get_mut_or_insert_with<F>(&mut self, query_key: K, make_val: F) -> &mut V
    where
        F: FnOnce() -> V,
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

impl<K, V> Default for PageCache<K, V>
where
    K: PartialEq + Ord,
{
    fn default() -> Self {
        Self {
            keys: Vec::new(),
            vals: Vec::new(),
        }
    }
}

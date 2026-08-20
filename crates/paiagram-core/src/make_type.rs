macro_rules! make_type {
    (
        $(#[$struct_attr:meta])*
        $struct_name:ident,
        data { $(
            $(#[$field_attr:meta])*
            $field_name:ident: $field_type:ty,
        )* }
        cache { $(
            $(#[$cache_attr:meta])*
            $cache_name:ident: $cache_type:ty,
        )* }
    ) => {
        paste::paste! {
            #[derive(Serialize, Deserialize, Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
            $(#[$struct_attr])*
            pub struct [<$struct_name Key>](std::num::NonZeroU64);

            pub type [<$struct_name KeyHashMap>]<T> = nohash_hasher::IntMap<[<$struct_name Key>], T>;
            pub type [<$struct_name KeyHasher>] = BuildNoHashHasher<[<$struct_name Key>]>;

            impl nohash_hasher::IsEnabled for [<$struct_name Key>] {}

            static [<$struct_name:snake:upper _COUNTER>]: AtomicU16 = AtomicU16::new(0);

            impl [<$struct_name Key>] {
                pub fn new() -> Self {
                    use web_time::SystemTime;
                    let now_ms = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;
                    let timestamp_48 = now_ms & 0xFFFF_FFFF_FFFF;
                    let counter_16 = [<$struct_name:snake:upper _COUNTER>]
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let mut raw_id = (timestamp_48 << 16) | (counter_16 as u64);
                    // I hope nobody would use this app and generate a key
                    // at exactly Jan 1, 1970 UTC+0...
                    if raw_id == 0 {
                        raw_id = 1;
                    }
                    Self(std::num::NonZeroU64::new(raw_id).unwrap())
                }
            }

            impl Key for [<$struct_name Key>] {
                fn to_bits(self) -> u64 {
                    self.0.get()
                }
            }

            #[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
            struct [<$struct_name Handle>](usize);

            // View stays raw data, as it's just used for passing data in/out
            #[derive(Clone, Debug, PartialEq)]
            pub struct [<$struct_name View>] {
                $(
                    $(#[$field_attr])*
                    pub $field_name: $field_type,
                )*
                $(
                    $(#[$cache_attr])*
                    pub $cache_name: $cache_type,
                )*
            }

            #[derive(Serialize, Deserialize, Clone, Debug)]
            pub struct [<$struct_name Info>] {
                $( pub $field_name: $field_type, )*
            }

            impl From<[<$struct_name Info>]> for [<$struct_name View>] {
                fn from(value: [<$struct_name Info>]) -> Self {
                    Self {
                        $( $field_name: value.$field_name, )*
                        $( $cache_name: <$cache_type as Default>::default(), )*
                    }
                }
            }

            impl From<[<$struct_name View>]> for [<$struct_name Info>] {
                fn from(value: [<$struct_name View>]) -> Self {
                    Self {
                        $( $field_name: value.$field_name, )*
                    }
                }
            }

            pub struct [<$struct_name Borrow>]<'a> {
                $( pub $field_name: &'a $field_type, )*
                $( pub $cache_name: &'a $cache_type, )*
            }

            pub(crate) struct [<$struct_name BorrowMut>]<'a> {
                $( pub $field_name: BorrowMutField<'a, $field_type>, )*
                $( pub $cache_name: BorrowMutField<'a, $cache_type>, )*
            }

            // The Struct wraps the entire collections in Arc
            #[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
            pub struct [<$struct_name Collection>] {
                registry: std::sync::Arc<[<$struct_name KeyHashMap>]<[<$struct_name Handle>]>>,
                keys: std::sync::Arc<Vec<[<$struct_name Key>]>>,
                $( $field_name: std::sync::Arc<Vec<$field_type>>, )*
                $(
                    #[serde(skip)]
                    $cache_name: std::sync::Arc<Vec<$cache_type>>,
                )*
            }

            impl [<$struct_name Collection>] {
                /// How many elements of this type currently exist in the world
                pub fn len(&self) -> usize {
                    self.registry.len()
                }

                fn get_handle(&self, key: [<$struct_name Key>]) -> Option<[<$struct_name Handle>]> {
                    self.registry.get(&key).cloned()
                }

                /// Check if the current collection contains the key
                pub fn contains_key(&self, key: [<$struct_name Key>]) -> bool {
                    self.registry.contains_key(&key)
                }

                /// Iterate over the keys in the collection.
                pub fn keys(&self) -> impl Iterator<Item = [<$struct_name Key>]> + '_ {
                    self.keys.iter().copied()
                }

                /// Remove an entry from the collection
                pub fn remove(&mut self, key: [<$struct_name Key>]) -> Option<[<$struct_name View>]> {
                    let registry_mut = std::sync::Arc::make_mut(&mut self.registry);
                    let handle = registry_mut.remove(&key)?;
                    let idx = handle.0;

                    let keys_mut = std::sync::Arc::make_mut(&mut self.keys);
                    let last_idx = keys_mut.len() - 1;
                    let last_key = keys_mut[last_idx];

                    let ret = [<$struct_name View>] {
                        $( $field_name: std::sync::Arc::make_mut(&mut self.$field_name).swap_remove(idx), )*
                        $( $cache_name: std::sync::Arc::make_mut(&mut self.$cache_name).swap_remove(idx), )*
                    };

                    keys_mut.swap_remove(idx);

                    if idx != last_idx {
                        registry_mut.insert(last_key, [<$struct_name Handle>](idx));
                    }

                    Some(ret)
                }

                pub fn insert(
                    &mut self,
                    key: [<$struct_name Key>],
                    view: [<$struct_name View>]
                ) -> Option<[<$struct_name View>]> {
                    let old_view = if self.registry.contains_key(&key) {
                        self.remove(key)
                    } else {
                        None
                    };

                    let registry_mut = std::sync::Arc::make_mut(&mut self.registry);
                    let keys_mut = std::sync::Arc::make_mut(&mut self.keys);

                    let idx = keys_mut.len();
                    registry_mut.insert(key, [<$struct_name Handle>](idx));
                    keys_mut.push(key);

                    $(
                        std::sync::Arc::make_mut(&mut self.$field_name).push(view.$field_name);
                    )*
                    $(
                        std::sync::Arc::make_mut(&mut self.$cache_name).push(view.$cache_name);
                    )*

                    old_view
                }

                pub fn query<'a, R>(
                    &'a self,
                    key: [<$struct_name Key>],
                    f: impl FnOnce([<$struct_name Borrow>]<'a>) -> R
                ) -> Option<R> {
                    let handle = self.get_handle(key)?;
                    let idx = handle.0;

                    let borrow = [<$struct_name Borrow>] {
                        $( $field_name: &self.$field_name[idx], )*
                        $( $cache_name: &self.$cache_name[idx], )*
                    };

                    Some(f(borrow))
                }

                /// Write access via a named-field struct
                fn update<R>(
                    &mut self,
                    key: [<$struct_name Key>],
                    f: impl FnOnce([<$struct_name BorrowMut>]) -> R
                ) -> Option<R> {
                    let handle = self.get_handle(key)?;
                    let idx = handle.0;

                    let borrow_mut = [<$struct_name BorrowMut>] {
                        $( $field_name: BorrowMutField {
                            borrow: &mut self.[<$field_name>],
                            idx
                        }, )*
                        $( $cache_name: BorrowMutField {
                            borrow: &mut self.[<$cache_name>],
                            idx
                        }, )*
                    };

                    Some(f(borrow_mut))
                }
            }
        }
    };
}

pub(super) use make_type;

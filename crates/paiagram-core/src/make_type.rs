macro_rules! make_type {
    {
        $(#[$struct_attr:meta])*
        $struct_name:ident,
        $(key = $key_impl:ty,)?
        get_name = $get_name_path:expr,
        data { $(
            $(#[$field_attr:meta])*
            $field_name:ident: $field_type:ty,
        )* }
        cache { $(
            $(#[$cache_attr:meta])*
            $cache_name:ident: $cache_type:ty,
        )* }
    } => { paste::paste! {
        make_type!(@make_key_impl $struct_name $($key_impl)?);
        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
        $(#[$struct_attr])*
        pub struct $struct_name {
            $( pub $field_name: $field_type, )*
        }

        #[derive(Clone, Debug, Default, PartialEq)]
        pub struct [<$struct_name Cache>] {
            $( pub $cache_name: $cache_type, )*
        }

        type [<$struct_name Collection>] = imbl::OrdMap<
            [<$struct_name Key>],
            WorldFieldContainer<$struct_name, [<$struct_name Cache>]>
        >;

        impl std::fmt::Debug for [<$struct_name Key>] {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let Some(world) = crate::WORLD_BACKUP.load_full() else {
                    return Ok(());
                };
                let Some(name) = ($get_name_path)(&*world, self) else {
                    return Ok(());
                };
                write!(f, "{name}")
            }
        }
    } };
    (@make_key_impl $struct_name:ident) => { paste::paste! {
        #[derive(Serialize, Deserialize, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
        pub struct [<$struct_name Key>](std::num::NonZeroU64);

        pub type [<$struct_name KeyHashMap>]<T> = nohash_hasher::IntMap<[<$struct_name Key>], T>;
        pub type [<$struct_name KeyHasher>] = BuildNoHashHasher<[<$struct_name Key>]>;

        impl nohash_hasher::IsEnabled for [<$struct_name Key>] {}

        static [<$struct_name:snake:upper _COUNTER>]: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);

        impl [<$struct_name Key>] {
            pub fn new() -> Self {
                Self(<Self as Key>::next_id(
                    &[<$struct_name:snake:upper _COUNTER>],
                    <Self as Key>::client_id(),
                ))
            }
        }

        impl Key for [<$struct_name Key>] {
            fn to_bits(self) -> u64 {
                self.0.get()
            }
        }
    } };
    (@make_key_impl $struct_name:ident $key_impl:ty) => { paste::paste! {
        pub type [<$struct_name Key>] = $key_impl;
    } };
}

pub(super) use make_type;

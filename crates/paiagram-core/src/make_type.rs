macro_rules! make_type {
    {
        $(#[$struct_attr:meta])*
        $struct_name:ident,
        $(key = $key_impl:ty,)?
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
    } };
    (@make_key_impl $struct_name:ident) => { paste::paste! {
        #[derive(Serialize, Deserialize, Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
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
    } };
    (@make_key_impl $struct_name:ident $key_impl:ty) => { paste::paste! {
        pub type [<$struct_name Key>] = $key_impl;
    } };
}

pub(super) use make_type;

// SPDX-License-Identifier: MPL-2.0
//! Key allocation.
//!
//! Keys pack a timestamp, a client id, and a per-key-type counter into the full `u64` range:
//!
//! ```text
//! | 42 bits: centiseconds since the Unix epoch | 6 bits: client | 16 bits: counter |
//! ```

use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

pub(crate) const COUNTER_BITS: u32 = 16;
pub(crate) const CLIENT_BITS: u32 = 6;
/// The low bits of a key that hold the per-centisecond counter.
pub(crate) const COUNTER_MASK: u64 = (1u64 << COUNTER_BITS) - 1;
/// The bits of a key that hold the client id.
pub(crate) const CLIENT_MASK: u64 = (1u64 << CLIENT_BITS) - 1;
const TIME_SHIFT: u32 = CLIENT_BITS + COUNTER_BITS;
/// The largest timestamp the 42-bit field can hold.
const MAX_TIME_CS: u64 = (1u64 << (64 - TIME_SHIFT)) - 1;
/// The largest key this allocator can produce, which is the full `u64` range.
pub(crate) const MAX_KEY: u64 =
    (MAX_TIME_CS << TIME_SHIFT) | (CLIENT_MASK << COUNTER_BITS) | COUNTER_MASK;

/// The largest client id that fits in the key's client field.
pub const MAX_CLIENT_ID: u8 = CLIENT_MASK as u8;

static CLIENT_ID: AtomicU8 = AtomicU8::new(0);

/// Sets the client id used by keys allocated afterwards.
///
/// The id must be unique among writers that may merge their worlds. Set it before
/// allocating keys; changing it later is safe but advances the timestamp by one
/// centisecond to avoid reusing counters.
pub fn set_client_id(client: u8) {
    assert!(client <= MAX_CLIENT_ID, "client id does not fit in 6 bits");
    CLIENT_ID.store(client, Ordering::Relaxed);
}

/// Returns the client id used by keys allocated afterwards.
pub fn client_id() -> u8 {
    CLIENT_ID.load(Ordering::Relaxed)
}

fn now_cs() -> u64 {
    (web_time::SystemTime::now()
        .duration_since(web_time::SystemTime::UNIX_EPOCH)
        .expect("the system clock is before the Unix epoch")
        .as_millis()
        / 10) as u64
}

/// Allocates the next key from `state` for `client`.
///
/// `state` holds the previous key; the compare-and-swap loop makes this safe to call from
/// multiple threads without a separate timestamp atomic.
pub(crate) fn next_id(state: &AtomicU64, client: u8) -> NonZeroU64 {
    let client = u64::from(client);
    debug_assert!(client <= CLIENT_MASK);

    loop {
        let old = state.load(Ordering::Relaxed);
        let last_time = old >> TIME_SHIFT;
        let last_client = (old >> COUNTER_BITS) & CLIENT_MASK;
        let last_counter = old & COUNTER_MASK;

        // If the client changes, advance the timestamp past every key allocated so far.
        // Otherwise this client could reuse counters from an earlier turn in the same
        // centisecond (e.g. client 0 -> client 1 -> client 0).
        let time = if client == last_client {
            now_cs().max(last_time)
        } else {
            now_cs().max(last_time + 1)
        };
        assert!(
            time <= MAX_TIME_CS,
            "key timestamp exceeds the 42-bit field"
        );

        let mut counter = if time == last_time && client == last_client {
            last_counter + 1
        } else {
            0
        };
        if counter > COUNTER_MASK {
            // This centisecond's counter is full; wait for the clock to advance.
            std::hint::spin_loop();
            continue;
        }
        // `NonZeroU64` cannot represent an all-zero time/client/counter tuple.
        if time == 0 && client == 0 && counter == 0 {
            counter = 1;
        }

        let raw = (time << TIME_SHIFT) | (client << COUNTER_BITS) | counter;
        assert!(raw <= MAX_KEY);
        if state.compare_exchange_weak(old, raw, Ordering::Relaxed, Ordering::Relaxed).is_ok() {
            return NonZeroU64::new(raw).expect("raw key is nonzero");
        }
    }
}

/// Returns the creation time encoded in a key.
pub(crate) fn creation_time(bits: u64) -> SystemTime {
    // One centisecond is ten milliseconds.
    SystemTime::UNIX_EPOCH + Duration::from_millis((bits >> TIME_SHIFT) * 10)
}

/// Returns the client id encoded in a key.
pub(crate) fn client(bits: u64) -> u8 {
    ((bits >> COUNTER_BITS) & CLIENT_MASK) as u8
}

/// Returns the per-centisecond counter encoded in a key.
pub(crate) fn generation(bits: u64) -> u16 {
    (bits & COUNTER_MASK) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_64_bits() {
        assert_eq!(MAX_KEY, u64::MAX);
    }

    #[test]
    fn keys_are_unique_and_monotonic() {
        let state = AtomicU64::new(0);
        let mut previous = 0;
        for _ in 0..10_000 {
            let bits = next_id(&state, 0).get();
            assert!(bits > previous, "keys must be strictly increasing");
            previous = bits;
        }
    }

    #[test]
    fn client_field_round_trips() {
        let state = AtomicU64::new(0);
        let key = next_id(&state, 7);
        assert_eq!(client(key.get()), 7);
    }

    #[test]
    fn changing_client_does_not_reuse_keys() {
        let state = AtomicU64::new(0);
        let mut previous = 0;
        for client in [0, 1, 0, 1, 0] {
            for _ in 0..8 {
                let bits = next_id(&state, client).get();
                assert!(bits > previous);
                previous = bits;
            }
        }
    }

    #[test]
    fn creation_time_is_now() {
        let state = AtomicU64::new(0);
        let key = next_id(&state, 0);
        let created = creation_time(key.get());
        let now = SystemTime::now();
        assert!(now.duration_since(created).unwrap() < Duration::from_secs(2));
    }
}

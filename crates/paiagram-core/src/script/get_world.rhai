// SPDX-License-Identifier: MPL-2.0
let world = get_world();
let alt_world = new_world();
// world.replace_with(alt_world);

// test commands
let hi_trip_key = new_trip_key();
world.add_trip(hi_trip_key, "Hi!", ());
world.rename_trip(hi_trip_key, "Bonjour!");
let bye_trip_key = new_trip_key();
let class_key = new_class_key();
world.add_trip(bye_trip_key, "Bye!", class_key);

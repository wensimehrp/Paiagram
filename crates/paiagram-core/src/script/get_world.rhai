// SPDX-License-Identifier: MPL-2.0
let main_world = get_world();
let alt_world = new_world();
main_world.replace_with(alt_world);

let keys = [];
for i in 0..=100 {
    keys.push(new_trip_key());
}

print(keys);

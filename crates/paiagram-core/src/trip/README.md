# Trip Model

The trip model in Paiagram is quite different from the model in qETRC or OuDia/OuDiaSecond. It
combines scheduling and routing. Each [`TripSchedule`] contains a list of entries. The vehicle must
visit every single node specified in its schedule.

Each entry contains two most basic properties: [`NodeKey`], which identifies the node the entry
passes, two [`TravelMode`]s that specifies how it visits the node, and [`TEntryId`], which
identifies the entry itself.

Each entry in the schedule enforces the vehicle on the trip to visit the node (a node can be either
a switch or a platform). In many cases, we might only know some intervals the vehicle traverses.
The [`TripSchedule::estimates`] method estimates a path and provide time information on when the
vehicle visits each node on the path. The estimated path is simply calculated using the Dijkstra's
algorithm. Thus, it is very likely that the actual running vehicle from real life would take a
different path. In this case, you can add additional [`TEntry`]s with the correct node and set the
entry's `arr_or_pass` field to [`TravelMode::Flexible`] and dep field to `None` to calibrate.

## Travel Modes

A [`TravelMode`] defines how the vehicle on the trip runs when travelling. There are three modes.
A _stable timepoint_ is a [`TravelMode`] accoponied by a [`TTime`] where the system is confident
that the vehicle must visit the node at the [`TTime`]:

- [`TravelMode::At`]: The vehicle must be at the location at a certain timepoint.
- [`TravelMode::For`]:
  - arr_or_pass: The vehicle must be at the location _after_ a duration since the previous _stable
    timepoint_.
  - dep: The vehicle must be at the location _after_ a duration since the arrival time. It cannot be
    a stable timepoint if the arrival mode is [`TravelMode::Flexible`].
- [`TravelMode::Flexible`]: The vehicle may be at the location at any given time between the
  previous and the next _stable timepoints_. It cannot be a stable timepoint.

The app also estimates the exact timepoint a TravelMode is at based on the context. For example:

| Entry Type | `arr_or_pass` | `dep`              | [`TEstimate`]                |
| ---------- | ------------- | ------------------ | ---------------------------- |
| Pinned     | At(10:00:00)  | Some(For(10mins))  | 10:00:00, 10:10:00           |
| Pinned     | For(30mins)   | None               | 10:40:00, 10:40:00           |
| Pinned     | For(10mins)   | Some(Flexible)     | 10:50:00, 10:50:00           |
| Derived    | /             | /                  | (estimate based on distance) |
| Pinned     | Flexible      | None               | (estimate based on distance) |
| Pinned     | For(1hr)      | Some(At(12:00:00)) | 11:50:00, 12:00:00           |
| Pinned     | Flexible      | Some(At(12:15:00)) | 12:15:00, 12:15:00           |
| Pinned     | Flexible      | Some(For(30mins) ) | (estimate based on distance) |
| Pinned     | At(13:00:00)  | None               | 13:00:00, 13:00:00           |

Some stable timepoints are trivial to calculate, such as the case of a Some(For) following an At;
others are impossbile to calculate. In such cases, the app uses an estimate based on the physical
distance between two stable timepoints' nodes, and the estimate is propotional to the
intervals' length.

## Valid Trips

Only some trip entry combinations are valid. Here are the rules:

- The first entry must contain at least one [`TravelMode::At`].
- The last entry must contain a [`TravelMode`] that
  is not [`TravelMode::Flexible`]

Every entry's node should also be able to reach the next node in the entry in the graph. Any invalid
combinations or unaccessible routes will result in missing time estimates, and a warning in the UI.

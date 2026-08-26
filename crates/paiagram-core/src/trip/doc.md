# Trip Model

The trip model in Paiagram is quite different from the model in qETRC or
OuDia/OuDiaSecond. It combines scheduling and routing. Each [`TripSchedule`]
contains a list of entries, and each entry can be one of the following:

- [`TEntry::Derived`]
- [`TEntry::Pinned`]: The vehicle must visit the station, and may or may not
  stop at the station.
- [`TEntry::PinnedNonStop`]: The vehicle must visit the station, and does not
  stop at the station.

Each entry contains two most basic properties: [`NodeKey`], which identifies the
node the entry passes, and [`TEntryId`], which identifies the entry itself.

A `Pinned` entry, including [`TEntry::Pinned`] and [`TEntry::PinnedNonStop`],
enforces the vehicle on the trip to visit the node (a node can be either a
switch or a platform). A [`TEntry::Derived`] entry, in contrast, is
automatically calculated by the system, and is filled into the scheudle to make
sure that the schedule form a valid, traversable path, where each
(TEntry, TEntry) pair's nodes can be found as an [`crate::IntervalKey`] in the
system's graph.

A [`TEntry::Derived`] entry is only a prediction, or an estimate, of the trip's
actual path. Derived entries are simply calculated using an A* algorithm from
[`petgraph::astar::astar`]. Thus, it is very likely that the actual running
vehicle from real life would take a different path. In this case, you can add
additional [`TEntry::PinnedNonStop`]s with the correct node and
[`TravelMode::Flexible`] to calibrate.

## Travel Modes

A [`TravelMode`] defines how the vehicle on the trip runs when travelling. There
are three modes. A _stable timepoint_ is a [`TravelMode`] accoponied by a
[`TTime`] where the system is confident that the vehicle must visit the node
at the [`TTime`]:

- [`TravelMode::At`]: The vehicle must be at the location at
  a certain timepoint.
- [`TravelMode::For`]:
  - arr ([`TEntry::Pinned`]), pass ([`TEntry::PinnedNonStop`]): The vehicle must
    be at the location _after_ a duration since the previous _stable timepoint_.
  - dep ([`TEntry::Pinned`]): The vehicle must be at the location _after_ a
    duration since the arrival time. It cannot be a stable timepoint if the
    arrival mode is [`TravelMode::Flexible`].
- [`TravelMode::Flexible`]: The vehicle may be at the location at any given time
  between the previous and the next _stable timepoints_. It cannot be a stable
  timepoint.

The app also estimates the exact timepoint a TravelMode is at based on the
context. [`TEntry`] For example:

| [`TEntry`]    | [`TravelMode`]                      | [`TEstimate`]                |
| ------------- | ----------------------------------- | ---------------------------- |
| Pinned        | arr: At(10:00:00), dep: For(10mins) | 10:00:00, 10:10:00           |
| PinnedNonStop | pass: For(30mins)                   | 10:40:00, 10:40:00           |
| Pinned        | arr: For(10mins), dep: Flexible     | 10:50:00, 10:50:00           |
| Derived       | (automatically Flexible)            | (estimate based on distance) |
| PinnedNonStop | pass: Flexible                      | (estimate based on distance) |
| Pinned        | arr: For(1hr), dep: At(12:00:00)    | 11:50:00, 12:00:00           |
| Pinned        | arr: Flexible, dep: At(12:15:00)    | 12:15:00, 12:15:00           |
| Pinned        | arr: Flexible, dep: For(30mins)     | (estimate based on distance) |
| PinnedNonStop | pass: At(13:00:00)                  | 13:00:00, 13:00:00           |

Some stable timepoints are trivial to calculate, such as the case of a For
following an At; others are impossbile to calculate. In such cases, the system
uses an estimate based on the physical distance between two stable timepoints'
nodes, and the estimate is propotional to the intervals' length.

## Valid Trips

Only some trip entry combinations are valid. Here are the rules:

- The first and last entries must be `Pinned`.
- The first entry (also `Pinned`) must contain at least one [`TravelMode::At`].
- The last entry (also `Pinned`) must contain a [`TravelMode`] that
  is not [`TravelMode::Flexible`]

Every entry's node should also be able to reach the next node in the entry in
the graph. Any invalid combinations or unaccessible routes will result in
missing time estimates, and a warning in the UI.

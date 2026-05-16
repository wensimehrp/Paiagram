#title[Trips and Vehicles]

= Trips

A trip is the basic unit in the network. Each trip has a class, associated vehicles, and an entry list which defines
which stations to visit.

A trip runs on the network graph. Its path is defined by its schedule. A schedule contains a set of trip entries, and
each of them has an arrival mode, a departure mode, and is associated with a station in the network graph. A trip could
never jump between two stations, unless there are absolutely no paths connecting those two stations, which is considered
an error.

Sometimes the trip does not take your desired route. In this case, you can add dummy entries with Non-stop arrival mode
and Flexible departure mode (see #link(<travel-mode>)[Travel Mode] for details).

= Travel Mode <travel-mode>

Each trip contains a set of entries. Each entry contains arrival and departure modes. A departure mode could be any of
the following:

#table(
  columns: 2,
  [At], [the trip departs at this specific timepoint],
  [For], [the trip departs after a given amount of time after arrival],
  [Flexible], [the departure time is flexible],
)

Likewise, an arrival mode could be any of the following:

#table(
  columns: 2,
  [At], [the trip arrives at this specific timepoint],
  [For], [the trip arrives after a given amount of time *after the previous stable timepoint*],
  [Flexible], [the arrival time is flexible],
  [Non-stop], [the trip does not stop at this station.],
)

Travel modes are designed to simplify work. For example, in cases where you only have an accumulated travel time for
specific intervals. You can easily convert between travel modes without losing work. You can also make it calculate the
intermediate time for you by setting travel modes to "flexible" or "non-stop".

= Timetabled, Fixed, and Derived Entries

Sometimes, the trip may only have some timetabled entries, and is only guaranteed to visit some stations. For example,
an express train might only have the times at the first and last stations, and the times between those stations, as well
as the path taken between those stations, are unknown. In this case, Paiagram would do the following:

- Calculate a shortest path between every entry pair in the timetable that doesn't have a direct path from the first to
  the second
- Assign that shortest path to the trip
- Automatically calculate the times at each intermediate station. Each intermediate station would generate a
  corresponding *derived* entry.

Each entry in the trip entry list falls into one of the following categories:

#table(
  columns: 3,
  table.header[Type][Description][Travel Modes],
  [Timetabled],
  [The trip would visit the entry's station at a determined time.],
  [All travel modes except (Flexible, Flexible)],

  [Fixed], [The trip is guaranteed to visit the station, but the exact time is uncertain.], [(Flexible, Flexible)],
  [Derived], [The trip is not guaranteed to visit this station.], [No travel modes],
)

You cannot edit the time of derived entries. However, it is possible to convert derived entries to fixed or timetabled
entries. In contrast, you can only delete timetabled and fixed entries.

== Editing the Trip's Path

You can edit the trip's path by inserting extra fixed (i.e. Flexible arrival mode, Flexible departure mode) entries.

= Vehicles

A vehicle is the "executor" of trips. Each vehicle runs a set of trips. Trips could be shared by multiple vehicles (as
in coupling and decoupling trains).

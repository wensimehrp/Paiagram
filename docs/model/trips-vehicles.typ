#import "../book.typ": book-page

#show: book-page.with(title: "Trips and Vehicles")

= Trips

A trip is the basic unit in organizing the network. Each trip has a class, associated vehicles, and a list of stations
to visit.

A trip runs on the network graph. Its path is defined by its schedule. A schedule contains a set of trip entries, and
each of them has an arrival mode, a departure mode, and a stop in the network graph. A trip could never jump between two
stops, unless there are absolutely no paths connecting those two stops (this is also considered an error).

Sometimes the trip does not take your desired route. In this case, you can add dummy entries with Non-stop arrival mode
and Flexible departure mode (see Travel Mode for details).

= Travel Mode

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
specific intervals. You can easily convert between travel modes without losing work.

= Vehicles

A vehicle is the "executor" of trips. Each vehicle runs a set of trips. Trips could be shared by multiple vehicles (as
in coupling and decoupling trains).

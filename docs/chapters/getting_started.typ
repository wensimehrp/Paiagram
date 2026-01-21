#import "mod.typ": *
= Getting Started

You can download Paiagram from #link("https://github.com/wensimerhp/paiagram/releases")[Github releases] or #link(
  "https://wensimehrp.github.io/Paiagram",
)[try it online]. The online version's performance would be worse compared to native versions, but the difference is
usually acceptable. You can also download the webpage and run it locally in your browser.

== The Model

Paiagram uses a graph-based system to store all stations and intervals between stations. All stations are represented as
nodes in the graph, and each interval is an edge between two nodes. Vehicles could traverse via edges, and visit nodes
along the way.

For performance sake, internally, the edges in Paiagram are _not_ directional, instead they are bi-directional. However,
you can manually specify the edge's direction.

For performance sake, internally, there could be _only one_ edge betwee two nodes. However, you can specify extra nodes
and connect from those nodes. Those extra nodes _would_ instead be of "waypoint" type.

The graph model allows easy manipulation of network, and allows for managing much larger networks. This also means that
Paiagram won't have the _Nobori/Kudari_ settings in OuDiaSecond, or _Shangxing/Xiaxing_ properties in qETRC/pyETRC &
friends.

In order to display a track section, you must specify a "Displayed Line". A displayed line holds a set of stations. The
distance between stations depends on the interval's length. Yet, it could always be manually adjsuted.

Labels, annotations, and lines on the diagram would automatically avoid colliding into each other, for better display.

Paiagram doesn't support directly exporting to PDF or printing, but you can export each page to Typst, and further
render that to SVG/PNG/PDF. The spacing on diagram pages would be reserved. Exporting _directly_ to PDF, and exporting
to LaTeX, docx, odt, etc. would never be supported.

== Vehicle-based model

Paiagram uses a vehicle-based system, that is, services are not the basic unit of the network, rather they are
properties attached to a vehicle's timetable entries.

The vehicle-based system cannot 100% emulate real-world systems. Besides the freight train network in Canada and U.S.,
which doesn't even have a schedule hence it is impossible to represent it in Paiagram, public transit networks such as
bus networks might go with a dispatch based system, i.e., which vehicle is running the service is not important, and it
is fine as long as there is a vehicle running. Take the TransLink's bus lines 15 and 50 as an example. This Metro
Vancouver transit company has a dispatch based bus system.

(TODO)

== Graphs, Diagrams, Vehicles

The graph is the top-level representation of the network. Each `.paiagram` file can hold one graph and one graph only.

The diagram is 1. a collection of vehicle times and 2. a visual representation of the graph. Each `.paiagram` file can
hold multiple diagrams. It is not recommended to have multiple diagrams for different types of services (e.g. local,
express, freight). Instead, keep all services that runs at the same time in the same diagram, and use filtering to only
show the services you want. The multi-diagram feature is mainly for representing the network's status in different
operation periods (e.g. normal operation, holiday schedule, special event schedule).

Vehicles are entities that traverse the graph. Each vehicle belongs to a diagram.

=== Interdiagram Vehicle Links and Patches

Vehicles can be linked across diagrams. This is useful when you have multiple diagrams for different operation periods,
and you want to represent a vehicle that runs across multiple operation periods. Sometimes a vehicle may have a very
small change in its timetable for one diagram, while other parts of the timetable remain the same. In this case, you can
use vehicle patches to only specify the changed parts, and link it to the original vehicle.

== Command Line Arguments

On top of using the graphical user interface, you can also use the command line interface on the desktop version for
quick opening,

== Arrival and Departure

Paiagram features 3 arrival types and 4 departure types:

- Arrival:

#table(
  columns: (auto, 1fr),
  [At], [The vehicle would arrive at the station AT the given time.],
  [For], [The vehicle would travel FOR the given amount of time between the current and the previous station.],
  [Flexible], [The arrival time is FLEXIBLE.],
)

- Departure:

#table(
  columns: (auto, 1fr),
  [At], [The vehicle would depart from the station AT the given time.],
  [For], [The vehicle would stay at the station FOR the given amount of time],
  [Flexible], [The departure time is FLEXIBLE],
  [Non-stop], [The vehicle DOES NOT STOP at this station],
)

The "At" type is the most common type you'd see on real-world timetables. If you have a train arriving at Shinagawa at
09:35:21, and departing at 09:35:41, you can specify its arrival and departure types as "At: 09:35:21" and "At:
09:35:41", respectively. Alternatively, if the stopping time is more important, and you don't want to manually calculate
the stopping time, you can specify the stopping time to be "For: 00:20". If you have a train departing from Stuttgart,
and you're not sure when it would depart, you can set the departure type as "Flexible", to avoid confusion.

When only the departure station and terminal stations' times are important, you can set the stations in between's time
to be "Flexible". Paiagram would handle them automatically.

== The Diagram

The diagram is the most important part in the program. You can open the diagram via the #kbd[Ctrl][D] shortcut.

== Vehicle Events

Vehicles can have events bound to a timetable entry. There are currently these events available:

- Composition of vehicles
- Decomposition of vehicles
- Loading/unloading passenger (or freight)

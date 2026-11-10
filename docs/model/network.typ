#title[The Network]

Paiagram's network is a graph, which contains nodes and intervals.

= Nodes

There are two types of nodes in Paiagram: *station nodes* and *intersection nodes*. Station nodes
are anything that is a part of a station the vehicle might stop and (un)load, or may use to bypass
the station. Station nodes include:

- Railway station platforms
- Railway station bypass tracks
- Road vehicle stops
- #link("https://en.wikipedia.org/wiki/Airport_apron")[Airport aprons]; helipads
- Docks, piers, berths.

Intersection nodes are anywhere three or more intervals intersect that are not station nodes. They
include:

- Road intersections
- Railway switches
- Airport taxiway and runway intersections
- #link("https://en.wikipedia.org/wiki/Fairway_(navigation)")[Shipping fairway intersections]
- Aviation waypoints and significant points; airway intersections.

Each node has a name and a coordinate. You may change a node's name and coordinate in the map tile.

= Intervals

An *interval* represents a connection between two nodes, which can be a physical road, airway, train
track, or fairway connecting two nodes. Vehicles may travel from one node to another one via
differnet intervals. An interval has the following:

- A list of coordinates that marks the interval's physical whereabouts.
- A direction marker, which can be either one way from and to either directions, or two way.
- An optional length field; if this field is omitted, Paiagram's internal calculations that require
  the distance of an interval will calculate the length based on the nodes connected by the interval
  and the interval's coordinates.

= Modelling

With station nodes, intersection nodes, and intervals, you can model entire railway networks in
Paiagram.

= Map System

Paiagram supports both flat and WGS84 coordinate systems. The flat coordinate system is best when
you are modelling a game, where the map itself is flat (e.g. A-Train, OpenTTD, Transport Fever);
WGS84 is best for processing real world datasets.

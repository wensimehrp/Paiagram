#title[The Network]

Paiagram's network is a graph, which contains nodes and intervals.

= Nodes

In Paiagram, there are two types of nodes: *station nodes* and *intersection nodes*. Station nodes
are anything that is a part of a station the vehicle might stop and (un)load, or may use to pass the
station. Station nodes include:

- Railway station platforms
- Railway station overpass tracks
- Road vehicle stops
- #link("https://en.wikipedia.org/wiki/Airport_apron")[Airport aprons]; helipads
- Docks, piers, berths.

Intersection nodes are anywhere three or more intervals intersect that are not station nodes. They
include:

- Road intersections
- Railway switches
- Taxiway intersections
- #link("https://en.wikipedia.org/wiki/Fairway_(navigation)")[Shipping fairway intersections]
- Aviation waypoints and significant points; airway intersections

= Intervals

An *interval* represents a connection between two nodes. Vehicles may travel from one node to
another one via differnet intervals.

= Modelling

With station nodes, intersection nodes, and intervals, you can model entire railway networks in
Paiagram.

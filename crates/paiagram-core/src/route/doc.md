# Route/Marey Chart

A route is the base of a [Marey
chart](https://en.wikipedia.org/wiki/Charles_Ibry). A Marey chart is essentially
a fancy time-distance graph with a bunch of lines on it, and each line
represents a trip. On the Marey chart, a line intersection means two vehicles on
the trip meet. If the route only has one lane (e.g. the case for single-track
railways), an intersection means a collision. If the route has two lanes, an
intersection is only a collision when the lines' slope are both positive or both
negative. The tool has been used in railway coordination for hundreds of years.

Despite the fact that most traffic networks are 3d (since they are in our
three-dimensional world), They can be almost always expressed in 2d form, with
the height information discarded. Marey charts take a step further by
compressing the 2d space into 1d space. In other words, coordinates are
simplified to milestones on a route. Of course, this model couldn't cover the
case where the network is very complex, where there might be multiple routes
connecting stations A and B, and an intersection between two trip lines might
not even mean the vehicles meet on a physical track (and this is why collision
detection has its separate processor in the intervals section.)

The route model in Paiagram is designed around Marey chart's model. Each
[`crate::RouteInfo`] has these components:

- name: The name of the route
- station records: A list of [`crate::RouteStationRecord`], and each station
  record contains:
  - The platforms in this record; either all or some platforms in the station
  - The milestone since the origin of the route; This is only for displaying the
    distance on the canvas. The canvas displays the shortest-path length if this
    field is not present.
  - Canvas length. Used for determining how tall (or long) the interval from the
    previous record to the current record should be on the canvas. The canvas
    length is calculated automatically using a log-based function if not
    provided.
  - Nodes included in the interval from the previous record to the current
    record.

## Milestone (Nominal Distance)

Station milestones might be different from the length of the actual tracks
connecting stations, and the milestone field accommodates that.

## Canvas Length

User setting. Doesn't affect data model. Canvas length only affects each
interval's appearence on the graph.

## Nodes

The nodes in the interval are the most important part of the route data model.
The canvas would use this progress info and display trip lines at different
sections of the diagram.

Each node has a progress, which is normalized to a 0..=1 range. Each starting
node gets a progress of 0, while each ending node gets 1. Nodes between the
starting node and the ending node gets different progress values based on their
distances to the source and target nodes. The progress is calculated as
P = Dₛ ÷ (Dₛ + Dₜ), where P is the progress between 0..=1, Dₛ is the shortest
distance from the node to any of the sources, and Dₜ is the shortest distance
from the node to any of the targets.

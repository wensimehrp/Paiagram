= Saving and Exporting

Paiagram supports saving and exporting to various formats

== Saving as `.paiagram`

The `.paiagram` format is the default saving format for Paiagram.

== Exporting to `.csv`

You can export single vehicles, single services, stations info, and interval info as `.csv` files. You cannot export the
entire graph as a `.csv` file. For information on exporting graphs, see @dot.

Subjects that can be exported:

- Vehicle schedule
  - With or without calculated times
- Service schedule
  - With or without calculated times
- Vehicle services
  - With or without calculated times
- List of stations
- List of intervals
- Tracks of a station

== Exporting to `.dot` <dot>

Graphs can be exported to GraphViz `.dot` files.

== Exporting to Typst code

You can export diagrams to Typst code, which can be then further processed and rendered via the Typst program.

== Exporting to JGRPP Orders

=== Notes and Tags

Notes attached to a timetable entry would be translated to labels in JGRPP. You can also specify the colour by adding a
`<colour>:(content)` prefix. For example:

- `red: Return to Paddington Station`
- `purple: Go to Shinkansen Centre`

all contain a valid colour tag.

=== Scheduled Dispatch

JGRPP features scheduled dispatch, a way to specify when a vehicle would depart from a station. Scheduled dispatch slots
could optionally have a tag. If they do have a tag, their

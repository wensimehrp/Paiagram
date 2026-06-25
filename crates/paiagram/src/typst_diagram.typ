// Typst diagram template
// Automatically size the page so the content fits. You can switch to a custom
// size for a more predictable result.
#set page(width: auto, height: auto)
// Load the data from the json file.
// By default, the export filename is "exported_diagram_data.json"
// If you changed the exported filename, you also need to change the name here,
// otherwise the document won't compile.
#let data = json("exported_diagram_data.json")
// Set the base unit. The x or the horizontal component controls the horizontal
// scale. The larger the x value is, the wider the graph goes, and vise versa,
#let base = (
  x: 10pt,
  // the same applies for the y component. The larger the y component is, the
  // taller the graph goes.
  y: 3pt,
)
// helper function and data that helps computation
#let rows = data.stations.slice(1).map(((_, it)) => it * base.y)
#let pts(d) = {
  (d.x * base.x, d.y * base.y)
}
// The main container for the graph. In this case, we are putting all lines and
// the grid inside a flexible box.
#box(width: auto, height: auto, clip: true, {
  // Renders each trip
  for trip in data.trips {
    for segment in trip.points {
      // The first segment needs to be handled independently because of typst
      // `curve` constraints.
      let ((a, b, c, d), ..rest) = segment
      let components = rest
        .map(((a, b, c, d)) => (
          curve.line(pts(a)),
          // Stopping lines are curved
          {
            let dx = c.x - b.x
            let dy = dx * 0.5
            let p = pts((
              x: b.x + dx / 2,
              y: b.y - dy,
            ))
            curve.cubic(pts(b), p, pts(c))
          },
          curve.line(pts(d)),
        ))
        .flatten()
      place(
        dy: -rows.first(),
        curve(
          stroke: color.rgb(..trip.color),
          curve.move(pts(a)),
          {
            let dx = c.x - b.x
            let dy = dx * 0.2
            let p = pts((
              x: b.x + dx / 2,
              y: b.y - dy,
            ))
            curve.cubic(pts(b), p, pts(c))
          },
          curve.line(pts(d)),
          ..components,
        ),
      )
    }
  }
  // Draw station and time grid
  grid(
    stroke: 1pt,
    rows: rows,
    columns: (50 * base.x,) * 24,
  )
})

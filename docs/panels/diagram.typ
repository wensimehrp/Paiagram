#import "../book.typ": book-page

#show: book-page.with(title: "Diagram")

The diagram tab displays a #link("https://en.wikipedia.org/wiki/Time%E2%80%93distance_diagram")[time--distance graph]
that can be used to track vehicles' relative positions at specific times. The horizontal axis is the time axis, and the
vertical axis is the distance axis. Each slanted polyline that goes across the horizontal station axes represent a trip
on the given interval.

= Controls

You can pan the viewport by pressing down arrow keys, scrolling on a touchpad, or dragging using your left key. You can
zoom in by scrolling while pressing `Ctrl`, or zoom by axis by scrolling while pressing `Ctrl`, and also pressing down
any of `Shift` and `Alt`. `Shift` zooms the horizontal axis, while `Alt` zooms the vertical axis.

You can zoom out to see a wider range of trips, or zoom in to see details of individual trips.

= On Your Screen

The diagram is essentially a time--distance graph. The horizontal axis is the time axis, and the vertical axis is the
distance axis. The horizontal axis's range is -365 days to 365 days, which should cover most diagrams' range.

Time markings are on the top. Station markings are on the left.

Each slanted line on the diagram represents a trip. The slanted line shows the trip's position at a given time. An
intersection in two lines going the same direction (up/down) signals a collision in the two trips.

You can click on each trip to edit its times. You can drag or click on handles on trip lines to shift the stop time or
modify the travel mode.

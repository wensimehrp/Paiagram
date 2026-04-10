#import "../book.typ": book-page

#show: book-page.with(title: "Diagram")

The diagram tab displays a #link("https://en.wikipedia.org/wiki/Charles_Ibry")[Marey diagram], which is fundamentally a
time--distance graph, that can be used to track vehicles' relative positions at specific times.

= On your screen

The horizontal axis is the time axis, and the vertical axis is the distance axis. Each slanted line that goes across the
horizontal station axes represent a trip on the given interval.

= Controls

You can pan the viewport by pressing down arrow keys, scrolling on a touchpad, or dragging using your left key. You can
zoom in by scrolling while pressing `Ctrl`, or zoom by axis by scrolling while pressing `Ctrl`, and also pressing down
any of `Shift` and `Alt`. `Shift` zooms the horizontal axis, while `Alt` zooms the vertical axis.

You can zoom out to see a wider range of trips, or zoom in to see details of individual trips.

= Editing

You can click on the coloured lines to edit the time of different trips. You can highlight trips by clicking on the
associated lines, and editing the trip's timetable by dragging the handles attached to each entry, or by clicking the
handles to open a popup, then editing the options provided in the popup.

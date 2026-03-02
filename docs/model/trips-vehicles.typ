#import "../book.typ": book-page

#show: book-page.with(title: "Trips and Vehicles")

A trip is the basic unit in organizing the network. Each trip has a class, associated vehicles, and a list of stations
to visit.

A vehicle is the "executor" of trips. Each vehicle runs a set of trips. Trips could be shared by multiple vehicles (as
in coupling and decoupling trains).

A trip runs on the network graph. Its path is defined by its schedule. A schedule contains a set of trip entries, and
each of them has an arrival mode, a departure mode, and a stop in the network graph. A trip could never jump between two
stops, unless there are absolutely no paths connecting those two stops (this is also considered an error).

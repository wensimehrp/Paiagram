#import "../book.typ": book-page
#import "@preview/shiroa:0.3.1": *

#show: book-page.with(title: "The Network")

Paiagram uses a #link("https://en.wikipedia.org/wiki/Graph_theory")[graph] to organize stations. Every single station is
a node in the graph. Stations are connected by one-way intervals. Intervals may have different lengths. To create a
two-way path connecting two stations, you must first create an interval going from station A to B, then another from
station B to A.

The graph is automatically managed by Paiagram. You can edit the graph in the #cross-link("/panels/map.typ")[Map panel]. You
can also export the graph to a #link("https://graphviz.org/")[Graphviz] `.dot` file for further processing.

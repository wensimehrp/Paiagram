#import "../book.typ": book-page

#show: book-page.with(title: "Map")

The map tab provides a global overview of your network. The tab is also the main interface for you to interact with your
network.

= Viewing



= Map Underlay

You can optionally enable a map underlay and display your network with the underlay. This may help visualizing and
planning the network. Currently supported underlays are:

- #link("https://openstreetmap.com/")[OpenStreetMap]
- #link("https://cyberjapandata.gsi.go.jp/")[Chiri-in Chizu (地理院地図)]
- #link("https://amap.com/")[Amap (AutoNavi)]

Please note that Amap uses #link("https://en.wikipedia.org/wiki/Restrictions_on_geographic_data_in_China")[GCJ-02]
coordinates. Both OpenStreetMap and Chiri-in Chizu use #link(
  "https://en.wikipedia.org/wiki/World_Geodetic_System",
)[WGS84] coordinates

The usage of map underlay services are subject to the corresponding service providers' terms and conditions.

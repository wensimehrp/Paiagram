#title[Map] <map>

The map tab provides a global overview of your network. The tab is also the main interface for you to interact with your
network.

= Viewing

Stations in your netowrk would be displayed as dots. The lines connecting stations are the intervals connecting the
stations. Trips are displayed as arrowheads traversing on intervals.

You can click on a dot to focus a station, a line to focus (an) interval(s), or an arrowhead to focus a trip. The
details would be displayed on the right panel.

= Map Underlay

You can optionally enable a map underlay and display your network with the underlay. This may help visualizing and
planning the network. Currently supported underlays are:

- #link("https://openstreetmap.com/")[OpenStreetMap]
- #link("https://https://www.arcgis.com/home/item.html?id=10df2279f9684e4a9f6a7f08febac2a9")[ESRI World Imagery]
- #link("https://cyberjapandata.gsi.go.jp/")[Chiri-in Chizu (地理院地図)]
- #link("https://amap.com/")[Amap (AutoNavi)]

Please note that Amap uses #link("https://en.wikipedia.org/wiki/Restrictions_on_geographic_data_in_China")[GCJ-02]
coordinates. OpenStreetMap, ESRI World Imagery, and Chiri-in Chizu use #link(
  "https://en.wikipedia.org/wiki/World_Geodetic_System",
)[WGS84] coordinates

*The usage of map underlay services are subject to the corresponding service providers' terms and conditions.*

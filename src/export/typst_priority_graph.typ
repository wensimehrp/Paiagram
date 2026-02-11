// Typst diagram template
// Automatically size the page so the content fits. You can switch to a custom
// size for a more predictable result.
#set page(width: auto, height: auto)
// Load the data from the json file.
// By default, the export filename is "exported_diagram_data.json"
// If you changed the exported filename, you also need to change the name here,
// otherwise the document won't compile.
#let data = json("exported_priority_data.json")
// Set the base unit. The x or the horizontal component controls the horizontal
// scale. The larger the x value is, the wider the graph goes, and vise versa,

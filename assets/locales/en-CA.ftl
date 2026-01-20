# Misc
-program-name = Paiagram

# Settings
settings-enable-romaji-search = Enable Romaji search
settings-show-performance-stats = Show performance analytics
settings-enable-autosave = Enable autosave
settings-autosave-interval = Autosave interval (minutes)
settings-enable-developer-mode = Enable Developer Mode

# Side panel
side-panel-edit = Edit
side-panel-details = Details
side-panel-export = Export

# fallback messages
side-panel-edit-fallback-1 = This tab hasn't implemented the {side-panel-edit} display yet.
side-panel-edit-fallback-2 = This is considered a bug. Feel free to open a ticket on GitHub!
side-panel-details-fallback-1 = This tab hasn't implemented the {side-panel-details} display yet.
side-panel-details-fallback-2 = This is considered a bug. Feel free to open a ticket on GitHub!
side-panel-export-fallback-1 = This tab hasn't implemented the {side-panel-export} display yet.
side-panel-export-fallback-2 = This is considered a bug. Feel free to open a ticket on GitHub!

# Tabs
# Start tab
tab-start = Start
tab-start-version = Version: {$version}
tab-start-revision = Revision: {$revision}
tab-start-description = A high-performance transport timetable diagramming and analysis tool built with egui and Bevy.
# Settings tab
tab-settings = Settings
# Diagram tab
tab-diagram = Diagram
tab-diagram-export-typst-diagram = Export to diagram (Typst)
tab-diagram-export-typst-diagram-desc = Export the current diagram to a Typst diagram. The exported diagram can be further customized in your preferred editor.
tab-diagram-export-typst-diagram-output = Typst output length: {$bytes} bytes
tab-diagram-export-typst-timetable = Export to timetable (Typst)
tab-diagram-export-typst-timetable-desc = Export the current diagram's timetable to a Typst timetable. The exported timetable can be further customized in your preferred editor.
tab-diagram-export-json-timetable = Export to timetable (JSON)
tab-diagram-export-json-timetable-desc = Export the current diagram's timetable to a JSON file. The exported timetable can be further processed with other tools.
# Graph tab
tab-graph = Graph
tab-graph-new-displayed-line = Create new displayed line
tab-graph-new-displayed-line-desc = Create a new displayed line. The new line would be used as the foundation of a diagram.
tab-graph-auto-arrange = Auto-arrange graph
tab-graph-auto-arrange-desc = Automatically arrange the graph using a force-directed layout algorithm. You can tweak the parameters below to adjust the layout.
tab-graph-auto-arrange-iterations = Iterations
tab-graph-arrange-via-osm = Arrange via OSM
tab-graph-arrange-button = Arrange
# tip: use local examples of area names
tab-graph-arrange-via-osm-desc = Use online sources to arrange the current the graph. This leverages OpenStreetMap data, and by clicking "{tab-graph-arrange-button}" you agree to OpenStreetMap's Terms of Use. You can query with an optional area name to limit the scope (e.g., Vancouver, Halifax).
tab-graph-arrange-via-osm-terms = Terms of Use
tab-graph-osm-area-name = Area filter:
tab-graph-animation = Animation controls
tab-graph-animation-desc = Animate trains on the graph.

# new lines desc
new-displayed-line = New Displayed Line

# general
copy-to-clipboard = Copy to Clipboard
done = Done
export = Export

# RW data
oud2-default-line = OUD2 Default Line
oud2-unnamed-line = Unnamed Line {$number}
oud2-unnamed-station = Unnamed Station {$number}
oud2-unnamed-diagram = Unnamed Diagram {$number}
oud2-unnamed-train = Unnamed Train {$number}

# Misc
-program-name = Paiagram

# Settings
settings-enable-romaji-search = Enable Romaji search
settings-show-performance-stats = Show performance analytics
settings-enable-autosave = Enable autosave
settings-autosave-interval = Autosave interval (minutes)
settings-enable-developer-mode = Enable Developer Mode
settings-preferences = Preferences
settings-dark-mode = Dark Mode
settings-language = Language
settings-project-settings = Project Settings

# Side panel
side-panel-edit = Edit
side-panel-details = Details
side-panel-export = Export

# fallback messages
side-panel-edit-fallback-1 = This tile hasn't implemented the {side-panel-edit} display yet.
side-panel-edit-fallback-2 = This is considered a bug. Feel free to open a ticket on GitHub!
side-panel-details-fallback-1 = This tile hasn't implemented the {side-panel-details} display yet.
side-panel-details-fallback-2 = This is considered a bug. Feel free to open a ticket on GitHub!
side-panel-export-fallback-1 = This tile hasn't implemented the {side-panel-export} display yet.
side-panel-export-fallback-2 = This is considered a bug. Feel free to open a ticket on GitHub!

# Tiles
# Start tile
tile-start = Start
tile-start-merge-stations-by-name = Merge stations by name
tile-start-amount-vehicles = Amount of vehicles:
tile-start-amount-trips = Amount of trips:
tile-start-amount-stations = Amount of stations:
tile-start-amount-platforms = Amount of platforms:
tile-start-amount-intervals = Amount of intervals:
tile-start-version = Version: {$version}
tile-start-revision = Revision: {$revision}
tile-start-description = A high-performance transport timetable diagramming and analysis tool built with egui and Bevy.
# Config tile
tile-config = Config
# Diagram tile
tile-diagram = Diagram
tile-diagram-save-typst-module = Save Typst module
tile-diagram-save-typst-module-desc = You must use the Typst module to render your JSON data.
tile-diagram-export-json-data = Export diagram as JSON
tile-diagram-export-json-data-desc = Export the current diagram to JSON.
tile-diagram-export-typst-diagram-output = Typst output length: {$bytes} bytes
tile-diagram-export-typst-timetable = Export to timetable (Typst)
tile-diagram-export-typst-timetable-desc = Export the current diagram's timetable to a Typst timetable. The exported timetable can be further customized in your preferred editor.
tile-diagram-export-json-timetable = Export to timetable (JSON)
tile-diagram-export-json-timetable-desc = Export the current diagram's timetable to a JSON file. The exported timetable can be further processed with other tools.
# Graph tile
tile-graph = Graph
tile-graph-new-displayed-line = Create new displayed line
tile-graph-new-displayed-line-desc = Create a new displayed line. The new line would be used as the foundation of a diagram.
tile-graph-auto-arrange = Auto-arrange graph
tile-graph-auto-arrange-desc = Automatically arrange the graph using a force-directed layout algorithm. You can tweak the parameters below to adjust the layout.
tile-graph-auto-arrange-iterations = Iterations
tile-graph-arrange-via-osm = Arrange via OSM
tile-graph-arrange-button = Arrange
tile-graph-arrange-mode-force = Force
tile-graph-arrange-mode-osm = OSM
tile-graph-arrange-progress = Arrange ({$mode}) progress: {$finished}/{$total} | retry queued: {$queued_retry}
# tip: use local examples of area names
tile-graph-arrange-via-osm-desc = Use online sources to arrange the current the graph. This leverages OpenStreetMap data, and by clicking "{tile-graph-arrange-button}" you agree to OpenStreetMap's Terms of Use. You can query with an optional area name to limit the scope (e.g., Vancouver, Halifax).
tile-graph-arrange-via-osm-terms = Terms of Use
tile-graph-osm-area-name = Area filter:
tile-graph-animation = Animation controls
tile-graph-animation-desc = Animate trains on the graph.
tile-graph-underlay-none = None
tile-graph-underlay-openstreetmap = OpenStreetMap
tile-graph-underlay-amap = Amap
tile-graph-underlay-chiriin = Chiri-in Chizu

# Trip tile
trip-table-station = Station
trip-table-arrival = Arrival
trip-table-departure = Departure

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

# Colours
colour-red      = Red
colour-orange   = Orange
colour-amber    = Amber
colour-yellow   = Yellow
colour-lime     = Lime
colour-green    = Green
colour-emerald  = Emerald
colour-teal     = Teal
colour-cyan     = Cyan
colour-sky      = Sky
colour-blue     = Blue
colour-indigo   = Indigo
colour-violet   = Violet
colour-purple   = Purple
colour-fuchsia  = Fuchsia
colour-pink     = Pink
colour-rose     = Rose
colour-slate    = Slate
colour-gray     = Gray
colour-zinc     = Zinc
colour-neutral  = Neutral
colour-stone    = Stone

# read files
read-file-prompt   = Read {$name}…
read-file-title    = Load {$name} files
read-file-filetype = {$name} Files

# Menu
menu-import-url-heading = Import from URL
menu-import-url-desc = Download the file from the Internet then import it into Paiagram
menu-url-label = URL:
menu-download-and-import = Download and Import
menu-route-timetable = Route Timetable
menu-new-route = New Route
menu-priority-graph = Priority Graph
menu-diagrams = Diagrams
menu-trips = Trips
menu-new-trip = New Trip
menu-text = Text
menu-new-text-message = New Text Message
menu-new-message = New Message
menu-project-remarks = Project remarks
menu-nothing-focused = Nothing focused
menu-more = More...
menu-fullscreen = Fullscreen
menu-import-url-prompt = Import from URL...
menu-save = Save...
menu-read = Read...
menu-load-save = Load Save
menu-paiagram-savefiles = Paiagram Savefiles
menu-save-ron = Save RON...
menu-read-ron = Read RON...
menu-load-ron-files = Load RON Files
menu-ron-files = RON Files
menu-about = About
menu-documentation = Documentation
menu-legal = Legal
menu-sync-system-clock = Sync with system clock
menu-maximized-view = Maximized view
menu-undo = Undo
menu-redo = Redo

# Classes Tile
tile-classes = Classes
classes-name = Class name
classes-count = Count
classes-color = Color

# Diagram Tile
diagram-export-oudia = Export to OuDia
diagram-use-global-timer = Use global timer
diagram-create-new-trip-scratch = Create a new trip from scratch
diagram-create-new-trip = Create a new trip
diagram-complete = Complete
diagram-find-route-between = Find a route between...
diagram-arrival-time = Arrival Time:
diagram-already-editing = Already editing...

# Graph Tile
graph-create-new-route = Create new route
graph-new-station = New Station

# Route Timetable Tile
tile-route-timetable = Route Timetable
route-timetable-sort-entries = Sort entries
route-timetable-stations = Stations

# Priority Graph Tile
tile-priority-graph = Priority Graph

# Settings Tile
settings-developer-mode = Developer Mode
settings-antialiasing-options = Antialiasing Options
settings-off = Off
settings-on = On
settings-lod-mode = LOD Mode
settings-lod-2x = 2×
settings-lod-4x = 4×

# Station Tile
tile-station = Station
station-include-non-stop = Include non-stop

# Text Tile
tile-text = Text message
text-markdown-hint = You may use markdown here

# Trip Tile
tile-trip = Trip

# Widgets
widget-at = At
widget-for = For
widget-flexible = Flexible
widget-non-stop = Non-stop

time-just-now = Just now

time-ago-seconds = { $count ->
    [one] {$count} second ago
   *[other] {$count} seconds ago
}
time-ago-minutes = { $count ->
    [one] {$count} minute ago
   *[other] {$count} minutes ago
}
time-ago-hours = { $count ->
    [one] {$count} hour ago
   *[other] {$count} hours ago
}
time-ago-days = { $count ->
    [one] {$count} day ago
   *[other] {$count} days ago
}

time-in-seconds = In { $count ->
    [one] {$count} second
   *[other] {$count} seconds
}
time-in-minutes = In { $count ->
    [one] {$count} minute
   *[other] {$count} minutes
}
time-in-hours = In { $count ->
    [one] {$count} hour
   *[other] {$count} hours
}
time-in-days = In { $count ->
    [one] {$count} day
   *[other] {$count} days
}

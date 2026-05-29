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
side-panel-edit-fallback-1 = This tab hasn't implemented the {side-panel-edit} display yet.
side-panel-edit-fallback-2 = This is considered a bug. Feel free to open a ticket on GitHub!
side-panel-details-fallback-1 = This tab hasn't implemented the {side-panel-details} display yet.
side-panel-details-fallback-2 = This is considered a bug. Feel free to open a ticket on GitHub!
side-panel-export-fallback-1 = This tab hasn't implemented the {side-panel-export} display yet.
side-panel-export-fallback-2 = This is considered a bug. Feel free to open a ticket on GitHub!

# Tabs
# Start tab
tab-start = Start
tab-start-merge-stations-by-name = Merge stations by name
tab-start-amount-vehicles = Amount of vehicles:
tab-start-amount-trips = Amount of trips:
tab-start-amount-stations = Amount of stations:
tab-start-amount-platforms = Amount of platforms:
tab-start-amount-intervals = Amount of intervals:
tab-start-version = Version: {$version}
tab-start-revision = Revision: {$revision}
tab-start-description = A high-performance transport timetable diagramming and analysis tool built with egui and Bevy.
# Settings tab
tab-settings = Settings
# Diagram tab
tab-diagram = Diagram
tab-diagram-save-typst-module = Save Typst module
tab-diagram-save-typst-module-desc = You must use the Typst module to render your JSON data.
tab-diagram-export-json-data = Export diagram as JSON
tab-diagram-export-json-data-desc = Export the current diagram to JSON.
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
tab-graph-arrange-mode-force = Force
tab-graph-arrange-mode-osm = OSM
tab-graph-arrange-progress = Arrange ({$mode}) progress: {$finished}/{$total} | retry queued: {$queued_retry}
# tip: use local examples of area names
tab-graph-arrange-via-osm-desc = Use online sources to arrange the current the graph. This leverages OpenStreetMap data, and by clicking "{tab-graph-arrange-button}" you agree to OpenStreetMap's Terms of Use. You can query with an optional area name to limit the scope (e.g., Vancouver, Halifax).
tab-graph-arrange-via-osm-terms = Terms of Use
tab-graph-osm-area-name = Area filter:
tab-graph-animation = Animation controls
tab-graph-animation-desc = Animate trains on the graph.
tab-graph-underlay-none = None
tab-graph-underlay-openstreetmap = OpenStreetMap
tab-graph-underlay-amap = Amap
tab-graph-underlay-chiriin = Chiri-in Chizu

# Trip tab
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

# Classes Tab
tab-classes = Classes
classes-name = Class name
classes-count = Count
classes-color = Color

# Diagram Tab
diagram-export-oudia = Export to OuDia
diagram-use-global-timer = Use global timer
diagram-create-new-trip-scratch = Create a new trip from scratch
diagram-create-new-trip = Create a new trip
diagram-complete = Complete
diagram-find-route-between = Find a route between...
diagram-arrival-time = Arrival Time:
diagram-already-editing = Already editing...

# Graph Tab
graph-create-new-route = Create new route
graph-new-station = New Station

# Route Timetable Tab
tab-route-timetable = Route Timetable
route-timetable-sort-entries = Sort entries
route-timetable-stations = Stations

# Priority Graph Tab
tab-priority-graph = Priority Graph

# Settings Tab
settings-developer-mode = Developer Mode
settings-antialiasing-options = Antialiasing Options
settings-off = Off
settings-on = On
settings-lod-mode = LOD Mode
settings-lod-2x = 2×
settings-lod-4x = 4×

# Station Tab
tab-station = Station
station-include-non-stop = Include non-stop

# Text Tab
tab-text = Text message
text-markdown-hint = You may use markdown here

# Trip Tab
tab-trip = Trip

# Widgets
widget-at = {"\uE65C"} At
widget-for = {"\uE12A"} For
widget-flexible = {"\uE6DE"} Flexible
widget-non-stop = {"\uE06C"} Non-stop

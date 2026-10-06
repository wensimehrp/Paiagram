// SPDX-License-Identifier: MPL-2.0

// config ui showcase

const text = (desc) => ({ type: "text", desc });
const separator = () => ({ type: "separator" });
const header = (variable, desc, items) => ({
    type: "collapsingHeader",
    variable,
    desc,
    items,
});
const radio = (variable, desc, items, def = 0) => ({
    type: "radio",
    variable,
    desc,
    items,
    default: def,
});
const tickbox = (variable, desc, def = false) => ({
    type: "tickbox",
    variable,
    desc,
    default: def,
});
const textInput = (variable, desc, placeholder, def = "") => ({
    type: "textInput",
    variable,
    desc,
    placeholder,
    default: def,
});
const dragValue = (variable, desc, min, max, def, integer = false) => ({
    type: "dragValue",
    variable,
    desc,
    integer,
    min,
    max,
    default: def,
});

function config_ui() {
    return [
        text(
            "Diagram Style Studio - a sample extension that exercises every config-UI widget.",
        ),
        text(
            'Adjust anything below, then press "Run script" to log the collected values.',
        ),
        separator(),

        header("canvas", "Canvas & layout", [
            radio(
                "orientation",
                "Reading direction",
                ["Left to right", "Top to bottom", "Free layout"],
                1,
            ),
            dragValue(
                "stationSpacing",
                "Station spacing (mm)",
                2.0,
                200.0,
                15.0,
            ),
            dragValue("margin", "Outer margin (px)", 0, 200, 12, true),
            tickbox("snapToGrid", "Snap to grid", true),
            dragValue("gridSize", "Grid step (px)", 1, 50, 5, true),
        ]),

        header("lines", "Lines & geometry", [
            dragValue("lineWidth", "Line width (mm)", 0.25, 10.0, 1.5),
            radio("lineCap", "End caps", ["Round", "Butt", "Square"], 0),
            tickbox("curvedCorners", "Round the corners at junctions", true),
            dragValue("cornerRadius", "Corner radius (mm)", 0.0, 40.0, 6.0),
            header("dashStyle", "Dashed styles", [
                tickbox("dashed", "Draw lines dashed", false),
                dragValue("dashLength", "Dash length (mm)", 1.0, 40.0, 8.0),
                dragValue("gapLength", "Gap length (mm)", 1.0, 40.0, 6.0),
            ]),
        ]),

        header("stations", "Stations & labels", [
            tickbox("drawTicks", "Draw a tick at each station", true),
            dragValue("tickLength", "Tick length (mm)", 0.0, 30.0, 4.0),
            separator(),
            tickbox("showNames", "Show station names", true),
            textInput("nameFont", "Font family", "e.g. Inter", "Inter"),
            dragValue("nameSize", "Font size (pt)", 6, 72, 13, true),
            radio(
                "namePlacement",
                "Placement",
                ["Above", "Below", "Left", "Right"],
                1,
            ),
            header("labelDetails", "Label details", [
                tickbox("bold", "Bold", false),
                tickbox("italic", "Italic", false),
                dragValue(
                    "letterSpacing",
                    "Letter spacing (px)",
                    -2.0,
                    10.0,
                    0.0,
                ),
                radio(
                    "caseStyle",
                    "Capitalisation",
                    ["As typed", "UPPER", "lower", "Title Case"],
                    3,
                ),
            ]),
        ]),

        header("timeAxis", "Time axis", [
            dragValue(
                "minutesPerCm",
                "Minutes per centimetre",
                0.5,
                120.0,
                5.0,
            ),
            radio(
                "timeFormat",
                "Time labels",
                ["24-hour", "12-hour", "Decimal hours", "Seconds"],
                0,
            ),
            tickbox("gridlines", "Hourly gridlines", true),
            tickbox("hourLabels", "Label every hour", true),
        ]),

        header("export", "Export", [
            textInput("filename", "File name", "diagram", "diagram"),
            radio("format", "Format", ["SVG", "PNG", "PDF", "WebP"], 0),
            dragValue(
                "rasterResolution",
                "Raster resolution (dpi)",
                72,
                1200,
                300,
                true,
            ),
            tickbox("transparentBackground", "Transparent background", false),
            textInput(
                "backgroundColor",
                "Background colour",
                "#ffffff",
                "#ffffff",
            ),
            separator(),
            text(
                "Rasterisation applies to PNG and WebP only; SVG and PDF stay vector.",
            ),
        ]),

        separator(),
        text(
            'This panel is the whole extension - press "Run script" and check the log for the values.',
        ),
    ];
}

function run() {
    const values = paiagram.return_values;

    // Values from inside a `collapsingHeader` are reachable as nested objects, e.g. the dashed-style
    // widget lives at `values.lines.dashStyle.dashed`, while a `radio` reads back as its index.
    const orientation = ["left to right", "top to bottom", "free"][
        values.canvas.orientation
    ];
    const format = ["SVG", "PNG", "PDF", "WebP"][values.export.format];
    paiagram.console.info(`Layout: ${orientation}; export format: ${format}.`);
    paiagram.console.info("Collected configuration:", values);
}

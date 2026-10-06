// SPDX-License-Identifier: MPL-2.0

function config_ui() {
    return [
        {
            type: "text",
            desc: "Export the focused trip's diagram lines as an SVG",
        },
        {
            type: "dragValue",
            variable: "minutesPerCm",
            desc: "Time scale",
            integer: false,
            min: 0.1,
            max: 120.0,
            default: 5.0,
        },
        {
            type: "text",
            desc: "minutes of timetable = 1 cm (5 = the app's default)",
        },
    ];
}

function run() {
    const selected = paiagram.world.selectedItems();

    // A focused trip serializes as `{ Trips: [tripKey] }` (SelectedItems::Trips).
    if (!selected || !selected.Trips || selected.Trips.length === 0) {
        paiagram.console.warn(
            "Focus a trip first — the current selection is not a trip",
        );
        return;
    }
    // `selectedItems()` emits the 64-bit key as a string, so it is exact.
    const tripKey = String(selected.Trips[0]);
    const minutesPerCm = Number(paiagram.return_values.minutesPerCm) || 5;

    // Shape: { tripKey: [ [ [ {arr, dep}, entry, index, progress ], ... ], ... ] } — index directly.
    const polylines = paiagram.world.diagramCache()[tripKey];
    if (!polylines || polylines.length === 0) {
        paiagram.console.warn(
            `Trip ${tripKey} has no diagram lines in the cache`,
        );
        return;
    }

    const svg = diagramToSvg(polylines, tripKey, minutesPerCm);
    paiagram.world.saveFile("focused_trip_diagram", ".svg", svg);
    paiagram.console.info(
        `Exported ${polylines.length} line(s) for trip ${tripKey} (${minutesPerCm} min/cm)`,
    );
}

// Mirrors the diagram's own drawing (`tabs/diagram.rs`, `gpu_trip.wesl`):
//   * each cached point is `(estimate, entry, idx, progress)`;
//   * a point is drawn as an arrival + departure at the same height, i.e. a horizontal dwell;
//   * its height is `mix(station_heights[idx], station_heights[idx + 1], progress)`, where every
//     interval's canvas length defaults to 1.5 cm = 15 mm, so it is `15 * (idx + progress)` mm.
//
// The axes use the diagram's fixed scales — `minutesPerCm` minutes span one centimetre
// horizontally, one millimetre of canvas is one millimetre vertically — and the whole thing is then
// fitted with a single uniform factor, so the lines keep their true shape.
function diagramToSvg(polylines, tripKey, minutesPerCm) {
    const PX_PER_CM = 96 / 2.54;
    const PX_PER_MM = PX_PER_CM / 10;
    const INTERVAL_MM = 15;
    const perCm = minutesPerCm > 0 ? minutesPerCm : 5;
    const xPerSecond = PX_PER_CM / (perCm * 60);
    const yPerMm = PX_PER_MM;

    // Vertices in physical space, and their tight bounding box.
    const raw = [];
    let minX = Infinity;
    let maxX = -Infinity;
    let minY = Infinity;
    let maxY = -Infinity;
    for (const line of polylines) {
        const points = [];
        for (const [estimate, _entry, idx, progress] of line) {
            const y = INTERVAL_MM * (idx + progress) * yPerMm;
            const a = estimate.arr * xPerSecond;
            const d = estimate.dep * xPerSecond;
            points.push([a, y], [d, y]);
            minX = Math.min(minX, a);
            maxX = Math.max(maxX, d);
            minY = Math.min(minY, y);
            maxY = Math.max(maxY, y);
        }
        if (points.length >= 2) raw.push(points);
    }
    if (raw.length === 0) {
        return '<svg xmlns="http://www.w3.org/2000/svg"></svg>';
    }
    if (!isFinite(minX)) minX = 0;
    if (!isFinite(maxX)) maxX = minX;
    if (!isFinite(minY)) minY = 0;
    if (!isFinite(maxY)) maxY = minY;

    const contentW = Math.max(1, maxX - minX);
    const contentH = Math.max(1, maxY - minY);

    // One uniform factor: the image keeps the physical x:y ratio ("minutes per centimetre").
    const MARGIN = 20;
    const TARGET = 1200;
    const scale = Math.min(
        (TARGET - MARGIN * 2) / contentW,
        (TARGET - MARGIN * 2) / contentH,
    );
    const width = contentW * scale + MARGIN * 2;
    const height = contentH * scale + MARGIN * 2;

    // Map the tight bounding box into [MARGIN, size - MARGIN]. The route start is at the top and
    // later stations go down (see `logical_y_to_screen_y`), so y maps straight downward.
    const tx = (x) => MARGIN + (x - minX) * scale;
    const ty = (y) => MARGIN + (y - minY) * scale;

    const paths = raw.map((points) => {
        let d = "";
        points.forEach(([x, y], i) => {
            d += `${i === 0 ? "M" : "L"} ${tx(x).toFixed(2)} ${ty(y).toFixed(2)} `;
        });
        return `<path d="${d.trim()}" />`;
    });

    return [
        `<?xml version="1.0" encoding="UTF-8"?>`,
        `<svg xmlns="http://www.w3.org/2000/svg" width="${width.toFixed(2)}" height="${height.toFixed(2)}" viewBox="0 0 ${width.toFixed(2)} ${height.toFixed(2)}">`,
        `<title>Diagram lines for trip ${tripKey}</title>`,
        `<rect width="100%" height="100%" fill="white" />`,
        `<g fill="none" stroke="black" stroke-width="1.5" stroke-linejoin="round" stroke-linecap="round">`,
        ...paths,
        `</g>`,
        `</svg>`,
    ].join("\n");
}

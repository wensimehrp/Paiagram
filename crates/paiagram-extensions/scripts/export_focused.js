// SPDX-License-Identifier: MPL-2.0

const DEFAULTS = Object.freeze({
    minutesPerCm: 5,
    strokeWidth: 1.5,
    maxSize: 1600,
});

// CSS pixels per centimetre. The SVG is fitted afterwards, so these are only used to keep the
// diagram's physical aspect ratio (minutes per centimetre horizontally, millimetres vertically).
const PX_PER_CM = 96 / 2.54;
const PX_PER_MM = PX_PER_CM / 10;
const INTERVAL_MM = 15;
const MARGIN = 20;

export function config_ui(_world) {
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
            default: DEFAULTS.minutesPerCm,
        },
        {
            type: "text",
            desc: "minutes of timetable = 1 cm (5 = the app's default)",
        },
        {
            type: "dragValue",
            variable: "strokeWidth",
            desc: "Line width (px)",
            integer: false,
            min: 0.1,
            max: 10.0,
            default: DEFAULTS.strokeWidth,
        },
        {
            type: "dragValue",
            variable: "maxSize",
            desc: "Maximum image size (px)",
            integer: true,
            min: 200,
            max: 8000,
            default: DEFAULTS.maxSize,
        },
        {
            type: "tickbox",
            variable: "transparent",
            desc: "Transparent background",
            default: false,
        },
    ];
}

export function run(world, params, io) {
    const selected = world.selectedItems();
    const tripKey = selected?.Trips?.[0];

    // A focused trip serializes as `{ Trips: [tripKey] }` (SelectedItems::Trips).
    if (tripKey === undefined) {
        console.warn("Focus a trip first. the current selection is not a trip");
        return;
    }

    // Keys are 64-bit BigInts. `diagramCache()` is a plain object keyed by their decimal strings.
    const tripKeyText = String(tripKey);
    const polylines = world.diagramCache()[tripKeyText];
    if (!Array.isArray(polylines) || polylines.length === 0) {
        console.warn(`Trip ${tripKeyText} has no diagram lines in the cache`);
        return;
    }

    const minutesPerCm = readNumber(
        params?.minutesPerCm,
        DEFAULTS.minutesPerCm,
        0.1,
        120,
    );
    const strokeWidth = readNumber(
        params?.strokeWidth,
        DEFAULTS.strokeWidth,
        0.1,
        10,
    );
    const maxSize = Math.round(
        readNumber(params?.maxSize, DEFAULTS.maxSize, 200, 8000),
    );
    const transparent = params?.transparent === true;

    const svg = diagramToSvg(polylines, {
        tripKey: tripKeyText,
        minutesPerCm,
        strokeWidth,
        maxSize,
        transparent,
    });
    io.saveFile(`focused_trip_${tripKeyText}.svg`, svg);
    console.info(
        `Exported ${polylines.length} line(s) for trip ${tripKeyText} ` +
            `(${minutesPerCm} min/cm)`,
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
function diagramToSvg(polylines, options) {
    const { tripKey, minutesPerCm, strokeWidth, maxSize, transparent } =
        options;
    const xPerSecond = PX_PER_CM / (minutesPerCm * 60);

    // Build vertices in physical space and their tight bounding box.
    const lines = [];
    let minX = Infinity;
    let maxX = -Infinity;
    let minY = Infinity;
    let maxY = -Infinity;

    for (const line of polylines) {
        if (!Array.isArray(line)) continue;

        const points = [];
        for (const point of line) {
            if (!Array.isArray(point) || point.length < 4) continue;

            const arr = Number(point[0]?.arr);
            const dep = Number(point[0]?.dep);
            const idx = Number(point[2]);
            const progress = Number(point[3]);
            if (![arr, dep, idx, progress].every(Number.isFinite)) continue;

            const y = INTERVAL_MM * (idx + progress) * PX_PER_MM;
            const arrival = arr * xPerSecond;
            const departure = dep * xPerSecond;
            points.push([arrival, y], [departure, y]);

            minX = Math.min(minX, arrival, departure);
            maxX = Math.max(maxX, arrival, departure);
            minY = Math.min(minY, y);
            maxY = Math.max(maxY, y);
        }
        if (points.length >= 2) lines.push(points);
    }

    if (lines.length === 0) {
        return emptySvg(tripKey, strokeWidth, transparent);
    }

    if (!Number.isFinite(minX)) minX = 0;
    if (!Number.isFinite(maxX)) maxX = minX;
    if (!Number.isFinite(minY)) minY = 0;
    if (!Number.isFinite(maxY)) maxY = minY;

    const contentW = Math.max(1, maxX - minX);
    const contentH = Math.max(1, maxY - minY);

    // One uniform factor: the image keeps the physical x:y ratio ("minutes per centimetre").
    const inner = Math.max(1, maxSize - MARGIN * 2);
    const scale = Math.min(inner / contentW, inner / contentH);
    const width = contentW * scale + MARGIN * 2;
    const height = contentH * scale + MARGIN * 2;

    // Map the tight bounding box into [MARGIN, size - MARGIN]. The route start is at the top and
    // later stations go down (see `logical_y_to_screen_y`), so y maps straight downward.
    const tx = (x) => MARGIN + (x - minX) * scale;
    const ty = (y) => MARGIN + (y - minY) * scale;

    const paths = lines
        .map((points) => pathData(points, tx, ty))
        .filter((d) => d.length > 0);

    const title = xmlEscape(`Diagram lines for trip ${tripKey}`);
    const desc = xmlEscape(
        `${lines.length} line(s), ${minutesPerCm} minutes per centimetre`,
    );
    const background = transparent
        ? ""
        : `<rect width="100%" height="100%" fill="#ffffff"/>`;

    return [
        `<?xml version="1.0" encoding="UTF-8"?>`,
        `<svg xmlns="http://www.w3.org/2000/svg" width="${fmt(width)}" height="${fmt(height)}" viewBox="0 0 ${fmt(width)} ${fmt(height)}" role="img" aria-labelledby="title desc">`,
        `<title id="title">${title}</title>`,
        `<desc id="desc">${desc}</desc>`,
        background,
        `<g fill="none" stroke="#000000" stroke-width="${fmt(strokeWidth)}" stroke-linejoin="round" stroke-linecap="round" shape-rendering="geometricPrecision">`,
        ...paths.map((d) => `<path d="${d}"/>`),
        `</g>`,
        `</svg>`,
    ]
        .filter((part) => part.length > 0)
        .join("\n");
}

function emptySvg(tripKey, strokeWidth, transparent) {
    const background = transparent
        ? ""
        : `<rect width="100%" height="100%" fill="#ffffff"/>`;
    return [
        `<?xml version="1.0" encoding="UTF-8"?>`,
        `<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1" viewBox="0 0 1 1" role="img" aria-label="${xmlEscape(`No diagram lines for trip ${tripKey}`)}" stroke-width="${fmt(strokeWidth)}">`,
        background,
        `</svg>`,
    ]
        .filter((part) => part.length > 0)
        .join("\n");
}

function pathData(points, tx, ty) {
    let d = "";
    let lastX = null;
    let lastY = null;

    for (const [x, y] of points) {
        const px = fmt(tx(x));
        const py = fmt(ty(y));
        if (px === lastX && py === lastY) continue;
        d += `${d.length === 0 ? "M" : " L"} ${px} ${py}`;
        lastX = px;
        lastY = py;
    }
    return d;
}

function fmt(value) {
    const rounded = Math.round(value * 100) / 100;
    return Object.is(rounded, -0) ? "0" : String(rounded);
}

function readNumber(value, fallback, min, max) {
    if (value === undefined || value === null || value === "") return fallback;
    const number = Number(value);
    if (!Number.isFinite(number)) return fallback;
    return Math.min(max, Math.max(min, number));
}

function xmlEscape(value) {
    return String(value).replace(
        /[&<>"']/g,
        (char) =>
            ({
                "&": "&amp;",
                "<": "&lt;",
                ">": "&gt;",
                '"': "&quot;",
                "'": "&apos;",
            })[char],
    );
}

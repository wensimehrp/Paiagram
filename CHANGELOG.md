# Paiagram Changelog

This is the changelog for Paiagram. Visit <https://paiagram.com> for the latest version and documentation.

## 0.1.2 (Apr. 23, 2026)

### Added

- Text tab.
- CommonMark rendering for text content in the Text tab.
- Station tab.
- Indicators when the time indicator is outside the viewport.

### Changed

- Diagram rendering now culls trips outside the viewport.
- Improved diagram line rendering quality/performance was with LOD and GPU-side optimizations.
- Diagram line antialiasing on all platforms.
  - Native always enables MSAA regardless of user preferences.
- Added trip travel duration display while extending a trip.
- Time drag input behavior improved (Adjusts by minutes by default).
- Timer moved to bottom and right panel removed.

### Fixed

- Saving issues that could prevent writing changes.
- Multiple OuDia import issues (including train/time handling).

### Breaking

- Dropped WebGL support.

## 0.1.1 (Mar. 24, 2026)

I forgot what I changed, but it was probably a minor bug fix or improvement.

## 0.1.0 (Mar. 17, 2026)

Initial release.

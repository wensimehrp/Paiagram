# Architecture

## Web deployment

- `web`: files related with web deployment.
- `web/main`: The main page of the website.
- `web/nightly`: The entrypoint of the nightly build. This folder is responsible for checking hardware specs and loading
  the module.

## Crates

All crates are located in the `crates` folder. Each crate is responsible for a specific functionality, and they are
organized as follows:

- `crates/paiagram-core`: The core crate of the project, containing the main logic, data structures, ECS systems and
components.
- `crates/paiagram-app`: The main entry point for the application.
- `crates/paiagram-pdf`: PDF export functionality.
- `crates/paiagram-raptor`: RAPTOR algorithm implementation for route planning.
- `crates/paiagram-rw`: Read and write functionality for various file formats.
- `crates/paiagram-soap`: SOAP algorithm implementation for transit-map style graph layout
- `crates/paiagram-ui`: UI components and systems for the application.
- `crates/epaint_default_fonts`: Default fonts for the application, used by the `paiagram-ui` crate. This crate replaces
  `epaint` to avoid shipping extra font files with the application.

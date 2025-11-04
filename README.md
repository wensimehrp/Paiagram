# Paiagram

Paiagram is an application for easy viewing, editing, and management of project diagrams.

## Why Paiagram, if OuDiaSecond/ETRC/qETRC/etc.?

OuDiaSecond (as well as OuDia) are there for several decades already. It is Windows-only, Japanese-only, _extremely_ slow on my machine (i9-13900HX w/ 64GB RAM), plus its MDI interface is outdated and clunky. I want something modern, cross-platform, and fast.

## Features

- Web deployment (WASM).

## Non-Features

- Printing support, yet typst export is planned.
  - Typst provide much better layout capabilities (and accessibility features) than what I can implement in Paiagram.
  - LaTeX export is not planned, as I don't want to deal with its awful syntax.

## Technologies

Paiagram is built using Rust. The GUI is built using egui, and the logic is powered by the bevy game engine.

## Licenses

Paiagram is licensed under GNU AGPLv3 or later. See LICENSE.txt for details.

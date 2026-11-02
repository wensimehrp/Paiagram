# Paiagram-OuDia

[OuDia] and OuDiaSecond r/w implementation in Rust. Also supports WebAssembly.

This crate helps parsing .oud/.oud2 formats used by timetabling tools
[OuDia](https://web.archive.org/web/20240909024820/https://take-okm.a.la9.jp/oudia/index.html)
and [OuDiaSecond](http://oudiasecond.seesaa.net/). This crate does not support
parsing WINDIA files.

This parses .oud/.oud2 strings into human readable intermediate
representation in plain, comprehensible English (as in the [`ir`] module).
The crate's goal is to provide a friendly interface for interacting with those
formats. The crate also provides serialization support from AST to .oud/.oud2
structure.

# Getting Started

To get started, simply use [`parse_oud2_to_ir`] for .oud2, or [`parse_oud_to_ir`]
for .oud.

Alternatively, you can use [`parse_to_ast`] if you want to parse a file to AST and
interact with the AST directly.

See <https://docs.rs/paiagram-oudia/> for documentation.

## Licensing

The tests are GPL-licensed material scraped from the original OuDiaSecond source
code. See `./tests` for more details.

The source code is available under the MPL-2.0 license.

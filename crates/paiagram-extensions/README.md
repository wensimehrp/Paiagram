# Extensions Module

Extensions add extra features to Paiagram.

## Languages

You can write extensions in [JavaScript](https://en.wikipedia.org/wiki/JavaScript), or any other language compiled to
[WebAssembly](https://en.wikipedia.org/wiki/WebAssembly).

## JavaScript

An extension defines two entry points:

- `config_ui()` declares the configuration UI, returning an array of elements for the host to
  render. It is evaluated when the extension's panel is opened and never commits world changes.
- `run()` is executed when the user runs the extension. It reads the values collected from the UI
  as `paiagram.return_values.<name>` and manipulates the world.

The host exposes its API on the `paiagram` global: `paiagram.world` is the current world and
`paiagram.console` forwards to the host log.

The program will automatically calculate the difference between the new world and the existing
world, and merge the results.

## WebAssembly

_WIP_

## Networking

_If your extension doesn't connect to the internet, you don't need to read this part!_

Networking is heavily restricted. Extensions that connects to the internet must specify a list of
domains they may connect to. There will be two phases when running such extensions:

- Networking phase; the extension may connect to a domain specified in its domain list and send or
  receive arbitrary data, but is not allowed to read any info from the world.
- Processing phase; in this phase, the extension may process the received data and manipulate
  the world based on the data. No networking activity is allowed in this phase.

## Writing to files.

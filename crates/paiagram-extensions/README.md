# Extensions Module

Extensions add extra features to Paiagram.

## Languages

You can write extensions in (Rhai)[https://rhai.rs], or any other language compiled to
(WebAssembly)[https://en.wikipedia.org/wiki/WebAssembly].

## Rhai

Rhai is a scripting langauge.

In order to manipulate the world, the extension must return a new world. The program will
automatically calculate the difference between the new world and existing world and merge the
results.

## WebAssembly

_WIP_

## Networking

_If your extension doesn't connect to the internet, you don't need to read this part!_

Networking is heavily restricted. Extensions that connects to the internet must specify a list of
domains it will connect. There will be two phases when running such extensions:

- Networking phase; the extension may connect to a domain specified in its domain
  list and send or receive arbitrary data, but is not allowed to read any info from the world.
- Processing phase; in this phase, the extension may process the received data and manipulate
  the world based on the data. No networking activity is allowed in this phase.

## Writing to files.

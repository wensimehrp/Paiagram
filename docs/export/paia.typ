#import "../book.typ": book-page

#show: book-page.with(title: "Exporting to .paia")

`.paia` (PAI-YAA) is the native file format.

You can save the entire project as `.paia` via the "Save..." button. Note that user preferences (e.g., language, dark
mode) are not saved.

= Processing `.paia`

`.paia` is essentially a LZ4 compressed CBOR file. It is a reflection of the system's state. You would have to first
decompress the binary (e.g. the `lz4` library for Python, or `lz4-wasm` for JavaScript), then use a CBOR parser (e.g.
`cbor2` for python, or `cbor` for JavaScript) to get its contents.

Think of CBOR as "the binary JSON".

The inner CBOR structure follows the #link("https://en.wikipedia.org/wiki/Entity_component_system")[ECS structure].
Nevertheless, all components are grouped by their related entities, which should make processing easier.

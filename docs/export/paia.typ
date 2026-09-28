#title[Exporting to .paia]

`.paia` (PAI-YAA) is the native file format.

You can save the entire project as `.paia` via the "Save..." button. Note that user preferences
(e.g., language, dark mode) are not saved.

= Processing `.paia`

`.paia` is essentially a zstd compressed CBOR file. It is a reflection of the system's state. You
would have to first decompress the binary, then use a CBOR parser (e.g. `cbor2` for python, or
`cbor` for JavaScript) to get its contents.

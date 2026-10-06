#title[Extensions]

Extensions extend Paiagram's features.

= Using Extensions

*Always check the source and content of any third-party content before running them!*

#let qjs-link = link("https://bellard.org/quickjs/")[QuickJS]
#let wasmi-link = link("wasmi-labs.github.io/wasmi/")[wasmi]

Extensions are executed in a #qjs-link or #wasmi-link sandbox for both web and native builds. As a
result, they are unlikely to harm your computer or perform malicious actions, but *there is still
risk to run them as extensions might exploit unknown vulnerabilities. Run extensions at your own
risk.*

You can get extensions at TODO. After downloading an extension, load them at TODO.

= Official Extensions

- Load station coordinates from OpenStreetMap
- Import data from OpenStreetMap
- Export line shape as SVG
- Bulk edit
- ...

= Developing Extensions

See TODO for details.

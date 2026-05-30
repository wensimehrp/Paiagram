#!/usr/bin/env -S typst compile --features bundle,html --format bundle
// nix develop ./docs/template#default --command typst watch --features bundle,html --format bundle ./docs/book.typ ./dist --root .

#import "template/lib.typ"
#import "./links.typ": links
#import "@preview/cmarker:0.1.8"

// temporary workaround so I don't need to write so many include statements
#let chapter-path(path) = lib.chapter(path, content: include path + ".typ")

#lib.book(
  title: "Paiagram " + links.paiagram-version + " Documentation",
  description: "Paiagram user documentation",
  canonical-url: "https://paiagram.com",
  root: "docs/",
  authors: ("Jeremy Gao",),
  language: "en",
  sidebar-image: html.img(
    src: "https://upload.wikimedia.org/wikipedia/commons/8/88/Thecanadiannearjasper.jpg",
  ),
  extra-head-content: {
    // tracking script
    html.elem(
      "script",
      attrs: (
        defer: "",
        src: "https://cloud.umami.is/script.js",
        data-website-id: "067cd05f-b395-4813-916c-2063c383685f",
      ),
    )
    // icon font
    html.link(
      rel: "stylesheet",
      type: "text/css",
      href: "https://cdn.jsdelivr.net/npm/@phosphor-icons/web@2.1.2/src/bold/style.css",
    )
  },
  debug: true,
  tree: (
    chapter-path("index"),
    chapter-path("tutorial"),
    [= Model],
    chapter-path("model/network"),
    chapter-path("model/trips-vehicles"),
    [= User Interface],
    chapter-path("panels/index"),
    chapter-path("panels/diagram"),
    chapter-path("panels/map"),
    chapter-path("panels/station"),
    [= Importing],
    chapter-path("import/qetrc"),
    chapter-path("import/oudia"),
    chapter-path("import/gtfs"),
    [= Exporting],
    chapter-path("export/paia"),
    chapter-path("export/oudia"),
    chapter-path("export/typst-diagram"),
    lib.separator(),
    chapter-path("misc/web"),
    lib.chapter("changelog", content: [
      #title[Changelog]
      #cmarker.render(label-prefix: "changelog-", read("../CHANGELOG.md"))
    ]),
    lib.chapter("license", content: [
      #title[License]

      This is the License of Paiagram #links.paiagram-version.

      #cmarker.render(label-prefix: "license-", read("../LICENSE.md"))
    ]),
    chapter-path("building"),
  ),
)

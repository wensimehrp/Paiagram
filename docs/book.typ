#!/usr/bin/env -S typst compile --features bundle,html --format bundle

#import "template/lib.typ"
#import "./links.typ": links

// temporary workaround so I don't need to write so many include statements
#let chapter-path(path) = lib.chapter(path, content: include path + ".typ")

#lib.book(
  title: "Paiagram " + links.paiagram-version + " Documentation",
  description: "Paiagram user documentation",
  canonical-url: "https://paiagram.com",
  root: "docs/",
  authors: ("Jeremy Gao",),
  language: "en",
  debug: true,
  tree: (
    lib.chapter("index", content: include "intro.typ"),
    chapter-path("import/qetrc"),
    chapter-path("import/oudia"),
    chapter-path("import/gtfs"),
    chapter-path("export/paia"),
    chapter-path("export/oudia"),
    chapter-path("export/typst-diagram"),
    chapter-path("model/network"),
    chapter-path("model/trips-vehicles"),
    chapter-path("panels/index"),
    chapter-path("panels/diagram"),
    chapter-path("panels/map"),
    chapter-path("misc/web"),
  ),
)

#import "@preview/haita:0.4.0": *
#import "./links.typ": links
#import "@preview/cmarker:0.1.10"

// temporary workaround so I don't need to write so many include statements
#let chapter-path(path) = chapter("docs/" + path, content: include path + ".typ")

#book(
  title: "Paiagram " + links.paiagram-version + " Documentation",
  description: "Paiagram user documentation",
  base-url: "https://paiagram.com",
  authors: ("Jeremy Gao",),
  lang: "en",
  html-renderer: new-hamber.html-renderer.with(
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
  ),
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
    divider(),
    chapter-path("misc/web"),
    chapter("docs/changelog", content: [
      #title[Changelog]
      #cmarker.render(label-prefix: "changelog-", read("../CHANGELOG.md"))
    ]),
    chapter("docs/license", content: [
      #title[License]

      This is the License of Paiagram #links.paiagram-version.

      #cmarker.render(label-prefix: "license-", read("../LICENSE.md"))
    ]),
    chapter-path("building"),
  ),
)

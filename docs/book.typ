#import "@preview/shiroa:0.3.1": *

#let links = (
  repo: "https://github.com/WenSimEHRP/Paiagram",
  app: "https://paiagram.com/nightly",
  discussions: "https://github.com/WenSimEHRP/Paiagram/discussions",
  discord: "https://discord.com/channels/142724111502802944/1281691431395790908",
  qq: "865211882",
  home: "https://paiagram.com"
)

#show: book

#book-meta(
  title: "Paiagram Docs",
  description: "Paiagram user documentation",
  repository: links.repo,
  discord: links.discord,
  authors: ("Jeremy Gao",),
  language: "en",
  summary: [
    #prefix-chapter("intro.typ")[Introduction]
    = Importing
    #chapter("import/qetrc.typ")[qETRC/pyETRC]
    #chapter("import/oudia.typ")[OuDia/OuDiaSecond]
    #chapter("import/gtfs.typ")[GTFS Static]
    = Exporting
    #chapter("export/paia.typ")[Paia (Native format)]
    #chapter("export/oudia.typ")[OuDia]
    #chapter("export/typst-diagram.typ")[Typst Diagram]
    = Model
    #chapter("model/network.typ")[Network]
    #chapter("model/trips-vehicles.typ")[Trips and Vehicles]
    = Panels
    #chapter("panels/index.typ")[Tiling System]
    #chapter("panels/diagram.typ")[Diagram]
    #chapter("panels/map.typ")[Map]
    = Miscellaneous
    #chapter("misc/linux-web.typ")[Linux Web]
  ],
)

// re-export page template
#import "./templates/page.typ": project
#let book-page = project

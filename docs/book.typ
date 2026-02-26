#import "@preview/shiroa:0.3.1": *

#show: book

#book-meta(
  title: "Paiagram Docs",
  description: "Paiagram user documentation",
  repository: "https://github.com/WenSimEHRP/Paiagram",
  discord: "https://discord.com/channels/142724111502802944/1281691431395790908",
  authors: ("Jeremy Gao",),
  language: "en",
  summary: [
    #prefix-chapter("intro.typ")[Introduction]
    = Importing
    #chapter("import/qetrc.typ")[qETRC/pyETRC]
    #chapter("import/oudia.typ")[OuDia/OuDiaSecond]
    #chapter("import/gtfs.typ")[GTFS Static]
    = Model
    #chapter("model/network.typ")[Network]
    = Panels
    #chapter("panels/diagram.typ")[Diagram]
    #chapter("panels/map.typ")[Map]
  ],
)

// re-export page template
#import "./templates/page.typ": project
#let book-page = project

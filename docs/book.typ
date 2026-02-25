
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
  ]
)



// re-export page template
#import "/templates/page.typ": project
#let book-page = project

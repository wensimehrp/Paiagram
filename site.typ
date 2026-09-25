#!/usr/bin/env bash
#let _ = ```sh
case "$1" in
  compile) typst compile --features bundle,html --format bundle $0 ;;
  watch)   typst watch   --features bundle,html --format bundle --pretty $0 ;;
  *)       echo "Unknown option: $1. Enter 'compile' or 'watch'"; exit 1 ;;
esac
exit 0
```

#import "@preview/typhoon:0.2.0"
#import "@preview/oxifmt:1.0.0": strfmt

#let realize(label, fn) = {
  let c = state("__realize-label+a-bunch-of-entropy", none)
  {
    show html.elem.where(tag: "a"): elem => c.update(href => elem.attrs.href)
    link(label)[]
  }
  context {
    fn(c.get())
  }
}

// documentation
#include "docs/book.typ"

// the real interesting stuff
#show html.elem: typhoon.update-elem
#import html as h
#import "@preview/based:0.2.0"
#import "docs/links.typ": links

#let icon-base64 = based.encode64(read("crates/paiagram/assets/paiagram-adaptive-no-bg.svg"))

#context [#asset("main-styles.css", typhoon.tailwind-css()) <main-styles>]

#document("index.html", h.html(lang: "en")[
  #h.head[
    #realize(<main-styles>, href => h.link(rel: "stylesheet", href: href))
    #h.meta(charset: "utf-8")
    #h.meta(name: "viewport", content: "width=device-width, initial-scale=1")
    #h.link(rel: "icon", type: "image/svg+xml", href: "data:image/svg+xml;base64," + icon-base64)
    #h.title("Paiagram")
    #h.style(
      ```css
      @import url('https://fonts.googleapis.com/css2?family=Lato:ital,wght@0,100;0,300;0,400;0,700;0,900;1,100;1,300;1,400;1,700;1,900&display=swap');
      :root {
        font-family: "Lato";
      }
      ```.text,
    )
  ]
  #h.body(class: "bg-white dark:bg-zinc-900")[
    // First section
    #h.section(
      class: "w-full p-5 -z-50 bg-emerald-700 dark:bg-emerald-900",
      h.div(
        class: {
          "mx-auto flex flex-col md:grid md:grid-cols-[0.7fr_1fr] text-white gap-10 p-5 md:p-10"
          " max-w-6xl"
        },
        {
          h.div(class: "text-5xl md:text-6xl font-bold flex flex-col justify-center gap-3")[
            #h.span(class: "text-shadow-sm")[Marey charts, reimagined.]
            #let base-classes = {
              "py-3 px-6 bg-gray-300/50 rounded-sm backdrop-blur-sm text-2xl w-full"
              " shadow-sm hover:shadow-lg hover:bg-black/30 transition-all"
              " flex flex-col [&>small]:text-sm"
            }
            #h.a(href: "https://example.com", class: base-classes, {
              h.span[Try it Online]
              h.small[Run the latest version]
            })
            #h.a(href: "docs/index.html", class: base-classes, {
              h.span[Read the Docs]
              h.small[Read the online documentation]
            })
            #h.a(href: links.repo, class: base-classes, {
              h.span[See the Source]
              h.small[Browse the source code]
            })
          ]
          h.div(
            class: "flex justify-center items-center",
            image(bytes(
              read("crates/paiagram/assets/paiagram-no-bg.svg").replace(
                "stroke=\"#000\"",
                "stroke=\"#fff\"",
              ),
            )),
          )
        },
      ),
    )
    // Explanation
    #h.article(class: "prose prose-zinc dark:prose-invert mx-auto my-10 max-w-3xl p-5 md:p-10")[
      #import "@preview/cmarker:0.1.10"
      #cmarker.render(read("README.md"), h1-level: 0)
    ]
  ]
])

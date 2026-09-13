#!/usr/bin/env bash
#let _ = ```sh
case "$1" in
  compile) typst compile --features bundle,html --format bundle $0 --root . ;;
  watch)   typst watch   --features bundle,html --format bundle --pretty $0 --root .;;
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
#import html: *

#context [
  #asset("main-styles.css", typhoon.tailwind-css()) <main-styles>
]

#document("index.html", html(lang: "en")[
  #head[
    #realize(<main-styles>, href => link(rel: "stylesheet", href: href))
    #meta(charset: "utf-8")
    #meta(name: "viewport", content: "width=device-width, initial-scale=1")
  ]
  #body(class: "relative")[
    // First section
    #section(
      class: "w-full p-5 -z-50",
      style: ```css
      background-image: url('https://raw.githubusercontent.com/wensimehrp/WenSimEHRP/refs/heads/main/_MG_3019.avif');
      background-size: cover;
      background-position: center;
      background-repeat: no-repeat;
      ```.text,
      div(
        class: {
          "mx-auto grid grid-cols-[0.7fr_1fr] text-white gap-10 p-10"
          " max-w-6xl"
        },
        {
          div(class: "text-6xl font-bold flex flex-col justify-center gap-3")[
            Marey charts, reimagined.
            #let base-classes = {
              "py-3 px-6 bg-white/50 rounded-sm backdrop-blur-sm text-2xl w-full"
              " shadow-sm hover:shadow-lg hover:bg-black/30 transition-all"
              " flex flex-col [&>small]:text-sm"
            }
            #a(href: "https://example.com", class: base-classes, {
              [Try it online]
              small[Run the latest version]
            })
            #a(href: "docs/index.html", class: base-classes, {
              [Read the Docs]
              small[Read the online documentation]
            })
          ]
          div(class: "mx-auto", image("crates/paiagram/assets/paiagram.svg"))
        },
      ),
    )
    // Explanation
    #article(class: "prose mx-auto mt-10")[
      #std.title[Paiagram]
      #std.link(std.label("page:/docs/index"))[Documentation]
      #lorem(1000)
    ]
  ]
])

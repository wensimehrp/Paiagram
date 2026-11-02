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

#let icon-base64 = based.encode64(read("/assets/paiagram-adaptive-no-bg.svg"))

#context [#asset("main-styles.css", typhoon.tailwind-css()) <main-styles>]

#document("index.html", h.html(lang: "en")[
  #h.head[
    #realize(<main-styles>, href => h.link(rel: "stylesheet", href: href))
    #h.meta(charset: "utf-8")
    #h.meta(name: "viewport", content: "width=device-width, initial-scale=1")
    #h.link(rel: "icon", type: "image/svg+xml", href: "data:image/svg+xml;base64," + icon-base64)
    #h.title("Paiagram")
    #h.elem("script", attrs: (
      defer: "",
      src: "https://umami-theta-pink.vercel.app/script.js",
      data-website-id: "958a845f-415f-459e-b5a5-7bae194ba9c2",
    ))
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
    #h.section(
      class: {
        "w-full p-5 bg-cover bg-center bg-no-repeat"
        " shadow-[inset_0_-4px_6px_-1px_rgba(0,0,0,0.1)]"
      },
      style: {
        "background-image: "
        "url('https://github.com/user-attachments/assets/a0449615-0bdd-43fe-8b6a-d100d4d5f410');"
      },
      h.div(
        class: "flex flex-col my-40 p-5 font-regular mx-auto items-center",
        {
          h.h1(class: "text-5xl md:text-8xl font-bold")[PAIAGRAM]
          h.h2(class: "text-4xl md:text-6xl")[Marey with ease]
        },
      ),
    )
    #h.div(class: {
      "flex flex-row flex-wrap px-5 my-10 max-w-5xl justify-between gap-5 mx-auto"
      " text-black dark:text-white"
    })[
      #let a = h.a.with(class: {
        "block flex-1 flex flex-col text-lg border-b-4 px-8 pb-2 pt-4 border-teal-700"
        " hover:bg-gray-500/50 whitespace-nowrap"
      })
      #a(href: "./latest-snapshot", {
        h.span[Try it Online]
        h.small[Run the latest snapshot]
      }) <online-version>
      #a(href: "docs/index.html", {
        h.span[Documentation]
        h.small[Read the docs]
      })
      #a(href: links.repo, {
        h.span[Repository]
        h.small[Browse the source]
      })
      #a(href: links.repo + "/discussions", {
        h.span[Discussions]
        h.small[Official discussion zone]
      })
    ]
    // Explanation
    #h.article(
      class: "prose prose-zinc dark:prose-invert mx-auto my-10 max-w-3xl p-5 md:p-10",
    )[
      #import "@preview/cmarker:0.1.10"
      #cmarker.render(read("README.md"), h1-level: 0)
    ]
  ]
]) <index>

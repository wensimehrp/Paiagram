#!/usr/bin/env bash
#let _ = ```sh
case "$1" in
  compile) typst compile --features bundle,html --format bundle $0 ; exit 0 ;;
  watch)   typst watch   --features bundle,html --format bundle --pretty $0 ; exit 0 ;;
  *)       echo "Unknown option: $1. Enter 'compile' or 'watch'"; exit 1 ;;
esac
exit 1
```

#import "@preview/typhoon:0.2.0"
#import "@preview/oxifmt:1.0.0": strfmt
#import "docs/links.typ": links
#import html as h
#import "@preview/based:0.2.0"

// the real interesting stuff
#show html.elem: typhoon.update-elem

#let icon-base64 = based.encode64(read("/assets/paiagram-adaptive-no-bg.svg"))

#document("index.html", h.html(lang: "en")[
  #h.head[
    #h.meta(charset: "utf-8")
    #h.meta(name: "viewport", content: "width=device-width, initial-scale=1")
    #h.link(rel: "icon", type: "image/svg+xml", href: "data:image/svg+xml;base64," + icon-base64)
    #h.title("Paiagram")
    #h.elem("script", attrs: (
      defer: "",
      src: "https://umami-theta-pink.vercel.app/script.js",
      data-website-id: "958a845f-415f-459e-b5a5-7bae194ba9c2",
    ))
    #context h.style(
      ```css
      @import url('https://fonts.googleapis.com/css2?family=Lato:ital,wght@0,100;0,300;0,400;0,700;0,900;1,100;1,300;1,400;1,700;1,900&display=swap');
      :root {
        font-family: "Lato";
      }
      ```.text
        + typhoon.tailwind-css(),
    )
  ]
  #h.body(class: "bg-zinc-100 dark:bg-zinc-800")[
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
        class: "flex flex-col my-40 p-5 font-regular mx-auto mx-auto max-w-6xl",
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
        "block flex-1 flex flex-col text-lg border-b-4 px-5 md:px-8 pb-2 pt-4 border-teal-700"
        " hover:bg-gray-500/50 whitespace-nowrap"
      })
      #a(href: links.latest-snapshot, {
        h.span[Try it Online]
        h.small[Run the latest snapshot]
      })
      #a(href: links.latest-snapshot-docs, {
        h.span[Documentation]
        h.small[Read the docs]
      })
      #a(href: links.repo, {
        h.span[Repository]
        h.small[Browse the source]
      })
      #a(href: links.discussions, {
        h.span[Discussions]
        h.small[Official discussion zone]
      })
    ]
    // Explanation
    #h.article(
      class: "prose prose-zinc dark:prose-invert mx-auto my-20 max-w-3xl px-5 md:px-10",
    )[
      #import "@preview/cmarker:0.1.10"
      #cmarker.render(read("README.md"), h1-level: 0)
    ]
    // gallery
    #let gallery-data = (
      "https://github.com/user-attachments/assets/1b53f533-ec6e-4183-b7d1-fbf471545318": [
        = Performant
        Run nation-wide datasets directly in Paiagram. Even in the browser.
      ],
      "https://github.com/user-attachments/assets/a0449615-0bdd-43fe-8b6a-d100d4d5f410": [
        = Intuitive
        Hover and select. No need to edit against a table, just drag stringlines in the diagram.
      ],
      "https://github.com/user-attachments/assets/81542102-28ed-42a6-a477-0a18765d5403": [
        = Dark Mode
        _Ohhhhhhhhh! It's so dark here!_
      ],
    )
    #for (idx, (img-src, desc)) in gallery-data.pairs().enumerate() {
      let flow-direction = if calc.rem(idx, 2) == 0 { "md:flex-row-reverse" } else { "md:flex-row" }
      h.div(class: "flex-col " + flow-direction + " mx-auto my-10 px-5 max-w-7xl flex gap-5", {
        h.section(class: "flex-1 prose prose-zinc dark:prose-invert p-3", desc)
        h.img(
          class: "flex-[2] bg-blue-200 min-w-0 object-cover object-center shadow-lg",
          src: img-src,
        )
      })
    }
  ]
]) <index>

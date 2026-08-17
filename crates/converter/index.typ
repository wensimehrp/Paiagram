#!/usr/bin/env bash
#let _ = ```sh
case "$1" in
  compile) typst compile --features html --format html $0 ;;
  watch)   typst watch   --features html --format html --pretty $0 ;;
  *)       echo "Unknown option: $1. Enter 'compile' or 'watch'"; exit 1 ;;
esac
exit 0
```
#import "@local/typhoon:0.2.0": *
#import html: *
#html(lang: "en", {
  head({
    meta(charset: "utf-8")
    meta(name: "viewport", content: "width=device-width,initial-scale=1")
    title[Paiagram OuDia Converter]
    import "@preview/based:0.2.0": base64
    let svg-icon = ```xml
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64">
      <text x="32" y="32" text-anchor="middle" dominant-baseline="central" font-size="56" fill="#000000"
        font-family="'Noto Color Emoji','Segoe UI Emoji','Apple Color Emoji',sans-serif">
          🔄
      </text>
    </svg>
    ```.text
    link(rel: "icon", type: "image/svg+xml", href: "data:image/svg+xml;base64," + base64.encode(svg-icon))
    context { style(tailwind-css()) }
    elem("base", attrs: (data-trunk-public-url: ""))
    elem("link", attrs: (data-trunk: "", rel: "rust", data-bin: "converter"))
  })
  show elem: update-elem
  body(class: "mx-auto max-w-5xl p-3 dark:bg-neutral-800 text-black dark:text-white", {
    h1(class: "font-bold text-xl mb-3")[OuDia(Second) Converter]
    noscript[This page requires JavaScript to function!]
    section(
      class: {
        "my-3 grid grid-cols-2 lg:grid-cols-4 gap-2 *:p-2 *:text-center *:border *:border-neutral-300 *:hover:bg-neutral-200"
        " *:dark:border-neutral-600 *:dark:hover:bg-neutral-600"
      },
      {
        elem("label", attrs: ("for": "output-format-select", class: "sr-only"))[Output format]
        select(id: "output-format-select", {
          option(value: "ast")[AST Debug Print]
          option(value: "json", selected: true)[JSON]
          option(value: "yaml")[YAML]
          option(value: "toml")[TOML]
          option(value: "ron")[RON]
        })
        elem("label", attrs: ("for": "file-upload"))[Load File]
        input(class: "hidden", type: "file", id: "file-upload", name: "File to convert", accept: ".oud,.oud2")
        button(id: "copy-output")[Copy Output]
        button(id: "download-output")[Download Output]
      },
    )
    div(class: "grid grid-cols-1 md:grid-cols-[1fr_1fr] gap-2 my-3", {
      let textarea-classes = {
        "w-full min-w-0 min-h-[50vh] px-1.5 py-1 font-mono text-sm border"
        " border-neutral-300 dark:border-neutral-600"
      }
      section({
        h2(class: "text-sm mb-1 font-bold")[Input]
        textarea(
          class: textarea-classes,
          id: "input-textarea",
          placeholder: "Paste raw .oud/.oud2 content here, or drag a file here.",
        )
      })
      section({
        h2(class: "text-sm mb-1 font-bold")[Output]
        textarea(
          class: textarea-classes + " bg-neutral-100 dark:bg-neutral-600",
          id: "output-textarea",
          disabled: true,
          placeholder: "Output will appear in this section.",
        )
      })
    })
    footer(class: "mt-3")[
      Made by Jeremy Gao.\
      #a(href: "https://github.com/wensimehrp/paiagram", class: "underline")[Source (github.com)]
    ]
  })
})

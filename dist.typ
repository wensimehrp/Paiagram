#!/usr/bin/env -S typst c --features bundle,html --format bundle
#import "@preview/typhoon:0.1.2": _plugin
#let tailwind-tracker = state("__tailwind-tracker", (:))
#show html.elem: it => {
  let classes = it.fields().attrs.at("class", default: ())
  if type(classes) == str {
    classes = classes.split(" ")
  }
  classes = classes.map(it => (it, none)).to-dict()
  tailwind-tracker.update(trk => trk + classes)
  it
}
#context asset("styles.css", {
  let classes = tailwind-tracker.final().keys().join(" ", default: "")
  let config = (:)
  let s = str(_plugin.generate(bytes(classes), cbor.encode(config)))
  s
})
#let page(page-path) = document(
  page-path.replace(".typ", ".html").replace("site/", ""),
  html.html(lang: "en", {
    import html: *
    head({
      meta(charset: "utf-8")
      meta(name: "viewport", content: "width=device-width, initial-scale=1")
      link(rel: "stylesheet", href: "/styles.css")
    })
    body({
      include page-path
    })
  }),
)

#page("site/index.typ")

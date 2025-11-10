#let style_html(page-content) = {
  set heading(numbering: "1.")
  page-content
}

#let style(page-content) = context {
  if target() == "html" {
    [You can also see the #link("./index.pdf")[PDF version].]
    html.link(rel: "stylesheet", href: "style.css")
    style_html(page-content)
    return
  }
  set page(
    paper: "a4",
    columns: 2,
    header: [_Paiagram Documentation_ #h(1fr) #datetime.today().display()],
    numbering: "1",
    margin: (
      x: .5in,
      y: .7in,
    ),
  )
  set par(justify: true, justification-limits: (
    tracking: (min: -0.02em, max: 0.03em),
  ))
  show <wip>: it => context {
    (
      counter(heading).display("1.")
        + h(
          .3em,
        )
        + it.body
        + pdf.artifact[~🚧]
    )
  }
  set text(lang: "en", region: "CA", font: ("Noto Serif", "Noto Serif CJK SC"))
  show heading: it => {
    set text(font: "Fira Sans")
    grid(
      columns: (auto, 1fr),
      gutter: 5pt,
      align: bottom,
      smallcaps(it), text(size: 8pt, fill: luma(70%), repeat[/]),
    )
  }
  show heading.where(level: 1): it => {
    colbreak(weak: true)
    it
  }
  set heading(numbering: "1.")
  show table.header: text.with(weight: "bold", font: "Fira Sans")
  show link: it => underline(
    stroke: luma(80%) + 4pt,
    background: true,
    offset: -.5pt,
    evade: false,
    text(font: "Fira Sans", it, weight: "bold"),
  )
  page-content
}

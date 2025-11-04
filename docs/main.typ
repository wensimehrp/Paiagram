#set page(
  paper: "a4",
  columns: 2,
  header: [_Paiagram Documentation_ #h(1fr) #datetime.today().display()],
  numbering: "1",
  margin: (
    x: .5in,
    y: .7in,
  ),
)
#set par(justify: true, justification-limits: (
  tracking: (min: -0.02em, max: 0.03em),
))
#show <wip>: it => context {
  (
    counter(heading).display("1.")
      + h(
        .3em,
      )
      + it.body
      + pdf.artifact[~🚧]
  )
}
#set text(lang: "en", region: "CA", font: ("Noto Serif", "Noto Serif CJK SC"))
#show heading: it => {
  set text(font: "Fira Sans")
  grid(
    columns: (auto, 1fr),
    gutter: 5pt,
    align: bottom,
    smallcaps(it), text(size: 8pt, fill: luma(70%), repeat[/]),
  )
}
#show heading.where(level: 1): it => {
  colbreak(weak: true)
  it
}
#set heading(numbering: "1.")
#show table.header: text.with(weight: "bold", font: "Fira Sans")
#import "chapters/mod.typ": *

#show link: it => underline(
  stroke: luma(80%) + 4pt,
  background: true,
  offset: -.5pt,
  evade: false,
  text(font: "Fira Sans", it, weight: "bold"),
)

#set document(
  author: "Jeremy Gao",
  date: datetime.today(),
  description: [Documentation for a transport timetable and routes visualization and management app, Paiagram],
  title: [Paiagram Documentation],
)

#pdf.attach("../LICENSE.md", description: "AGPLv3 License for Paiagram")

#outline(depth: 2)

#heading(outlined: false, numbering: none, bookmarked: false)[Before Reading]

If you see the sign 🚧, this chapter is a work in progress!

Text boxes that #kbd[look][like][this] stand for keyboard shortcuts. For example: #kbd[Ctrl][Alt][F2]

Text that #link("https://example.com")[looks like this] are links. You can click them to open the link in your default web browser.

Text boxes that #path[look][like][this] stand for file paths or directory paths. For example: #path[\~][.local][.config][Paiagram]

You can also read this manual in this language:

- #link("https://example.com")[Simplified Chinese]

#include "chapters/introduction.typ"
#include "chapters/getting_started.typ"
#include "chapters/interface.typ"
#include "chapters/foreign.typ"
#include "chapters/import_export.typ"
#include "chapters/extra_aid.typ"
#include "chapters/building.typ"

= External Resources

There aren't many external resources for Paiagram yet.

= Credits

- Developer: Jeremy Gao
- Documentation: Jeremy Gao
- Special thanks to:
  - x.e.p. for their wonderful work on qETRC/pyETRC, which inspired Paiagram.
  - Random people I spoke to on the internet for their feedback and suggestions.
  - Jason.

#outline(target: <wip>, title: [Work in Progress Chapters])

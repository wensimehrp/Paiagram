#import "style.typ"
#show: style.style
#import "chapters/mod.typ": *
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

Text that #link("https://example.com")[looks like this] are links. You can click them to open the link in your default
web browser.

Text boxes that #path[look][like][this] stand for file paths or directory paths. For example:
#path[\~][.local][.config][Paiagram]

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

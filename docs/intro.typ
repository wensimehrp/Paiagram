#import "./book.typ": book-page, links

#show: book-page.with(title: "Introduction")

Since you are looking at this page, I would assume you are interested in some aspects of transport. You might be
interested in railway locomotives, bento boxes, tracks and yards, or operation in general. So here is Paiagram, read it
with me -- PAI-YAA-GRAM. You can use Paiagram to figure out how to make transport timetables and visualize them.

Paiagram can help you on...:

- Making timetables for your model transportation system
- Visualizing and simulating vehicle movement at any given time point
- Diagnosing your timetables.
- Maybe more?

Paiagram is not an all-in-one tool where you would find every single feature related with timetabling, though. However,
if you do want something to be added into the program, #link("https://github.com/WenSimEHRP/Paiagram/issues")[feel free
  to open a ticket!]

= Getting Started

You can use the #link(links.home)[web version] or the desktop version. The web version runs on Chromium 113+. Latest
versions of Firefox and Safari are also supported. Despite the fact that Paiagram supports Linux and Windows, we do not
provide a pre-compiled version just yet. You would, unfortunately, have to compile the desktop version yourself.

If you have any trouble using the application, feel free to ask in #link(links.discussions)[GitHub discussions], or in
our QQ group chat: #raw(links.qq).

The performance gap between the web version and the desktop version when processing small datasets is usually
acceptable. To give you a rough idea of how performant the app is, here are some very inaccurate, for-reference-only
benchmarks:

- OuDiaSecond (Windows version, via Wine, which taxes the CPU a bit) runs at some PowerPoint level framerate on my
  i9-13900HX laptop after loading the `sample.oud2` file. The app lags whenever I scroll down the timetable or the
  diagram, regardless of how many items are displayed on the screen.
- This app runs at \~30fps on my classmate's Chromebook's Chrome browser after loading the same `sample.oud2` dataset.
  The app only lags when there are too many elements displayed on the screen, but even so the app still feels quite
  responsive.

In some cases, the web version runs faster than the native version (e.g. a poorly configured Linux laptop).

= Related Projects

Here are some related projects you might be interested in:

- #link(links.oudia)[OuDia]: Japanese diagramming tool created by take-okm.
- #link(links.oudia-second)[OuDiaSecond]: Japanese diagramming tool developed by diagram-mania. Successor of OuDia.
- #link(links.pyetrc)[pyETRC]: predecessor of qETRC.
- #link(links.qetrc)[qETRC] (#link(links.qetrc-docs)[documentation]): diagramming tool developed by x.e.p..
- #link(links.sono-sujiya)[Sono Sujiya]: versatile diagramming tool focusing on GTFS i/o.

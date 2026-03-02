#import "./book.typ": book-page, links

#show: book-page.with(title: "Introduction")

Since you are looking at this page, I would assume you are interested in some aspects of transport. You might be
interested in railway locomotives, bento boxes, tracks and yards, or operation in general. In Paiagram, we figure out
how to make transport timetables and visualize them.

Let's take a look at what Paiagram can do:

- Make timetables for your model transportation system
- Visualize and simulate vehicle movement at any given time point
- Diagnose your timetables.

Paiagram is not an all-in-one tool where you would find every single feature related with timetabling, though. However,
if you do want something to be added into the program, #link("https://github.com/WenSimEHRP/Paiagram/issues")[feel free
  to open a ticket!]

= Getting Started

If you have any trouble using the application, feel free to ask in #link(links.discussions)[GitHub discussions], or in
our QQ group chat: #raw(links.qq).

You can use the #link(links.home)[web version] or the desktop version. The web version runs on Chromium 113+. Latest
versions of Firefox and Safari are also supported. You would, unfortunately, have to compile the desktop version
yourself.

The performance gap between both versions when processing small datasets is usually acceptable. To give you a rough idea
of how performant the app is, here are some very inaccurate, for-reference-only benchmarks:

- OuDiaSecond (Windows version, via Wine, which taxes the CPU a bit) runs at some PowerPoint level framerate on my
  i9-13900HX laptop after loading the `sample.oud2` file. The app lags whenever I scroll down the timetable or the
  diagram, regardless of how many items are displayed on the screen.
- This app runs at \~30fps on my classmate's Chromebook's Chrome browser after loading the same `sample.oud2` dataset.
  The app only lags when there are too many elements displayed on the screen, but even so the app still feels quite
  responsive.

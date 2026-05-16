#title[The Tiling System]

Paiagram features a tiling system that allows you to conveniently split and organize your workspace.

= The Tiling System

The tiling system allows you to group multiple tabs or tab groups together. You can horizontally or vertically split
your workspace into many parts to quickly switch between tasks.

= Commands Panel

You can use the shortcut `Ctrl+P` to open the command panel, and quickly open tabs from the command panel. You can
search up station, route, and trip names from the command panel to quickly open station, route, and trip tabs.

== Searching Chinese and Japanese Names using Pinyin and Romaji

You can also search Chinese and Japanese using Pinyin, Pinyin acronyms, Double Pinyin, and Hepburn--style Romaji. For
example, all of those queries would match "北京西":

- `beijingxi` (Pinyin)
- `bjx` (Pinyin acronym)
- `bzj;xi` (Microsoft Double Pinyin)
- `pekinnishi` (Romaji)
- `pekin` (Romaji)

And all of those would match "山手線":

- `shanshouxian` (Pinyin)
- `ssx` (Pinyin acronym)
- `ujubxm` (Microsoft Double Pinyin)
- `yamate` (Romaji)
- `yamatesen` (Romaji)
- `yamanotesen` (Romaji)

Please note that the Romaji matcher has its limitations. There are so many readings for the same character, and the
system may fail when processing some specific character combinations. For example, `kasuga` could not match "春日", and
`haneda` won't match "羽田" (although in this case you could enter `haneten` instead).

This feature is supported by #link("https://github.com/Chaoses-Ib/ib-matcher")[the IB Matcher]. If you like this
feature, consider giving the author a star on GitHub!

= Assistance Panel / Right Panel

The right panel provides functions to edit your work and extra data that may help you analyzing your network and
timetable.

The assistance panel contains three tabs: Edit, Properties, and Export

== "Edit" Tab

== "Properties" Tab

== "Export" Tab

You can use the Export tab to export the current tab's information.

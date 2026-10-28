#title[Notes]

I've been building Paiagram since November 2025, but the idea of building a Marey chart editor had
sticked in my head for a much longer time. I have tried drawing my own Marey charts since grade
seven, but I soon found it a tedious task. I like the process of creating new symbols and adding new
annotations on the chart, not drawing more stringlines on my diagram.

#let ottd-link = link("https://openttd.org")[OpenTTD]
#let tpf3-link = link("https://www.transportfever3.com/")[Transport Fever]
#let a-train-link = link("https://en.wikipedia.org/wiki/A-Train")[A-Train]

I wouldn't even say that I am a railway fan; what I studied in high school and university were also
unrelated with trains or timetables#footnote[I never followed my timetables. I skipped a lot of
  classes.]. I don't know much about railway operations, timetables, dispatching, trains, etc., and
I only composed the features in the app based on what I saw from games such as #ottd-link,
#tpf3-link, #a-train-link, and prior art, including OuDia(Second) and qETRC.

So why did I write Paiagram, if I am not remotely related with railways? Beacuse I enjoy the process
of creating, and I like structured systems, including transport. It didn't matter if I lack a
specifc piece of knowledge required for implementing a specific feature, because I can always learn
as I write#footnote[
  Except sometimes it doesn't work. I think I'll have to learn Japanese to understand those
  mysterious railway forum posts.
]. It didn't matter if it takes a long time to finish the app. I enjoy the process anyways.

And I tried many new features in Paiagram. To name a few:

- egui for app UI
- bevy for state management
- imbl for state management
- multithreaded engine
- WebAssembly port

Some of which worked exceptionally well, though I am not sure if they introduce subtle bugs. Some of
which were abandoned (e.g. bevy engine) as the development moved on.

Paiagram might be very buggy, but it has so many interesting features that may#footnote[or may not]
help you when scheduling. Enjoy!#footnote[Or suffer!]

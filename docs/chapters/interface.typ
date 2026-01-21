#import "mod.typ": *
= Interface

The interface is designed to be intuitive and user-friendly. The workspace is split into two parts: the properties panel
on the left and the main canvas on the right.

The properties panel contains information and setting for the currently selected object. You can also multi-select
objects to edit their properties in bulk. Nevertheless, objects that don't share common properties cannot be edited in
bulk.

The main canvas is where you can visualize and edit your timetable graph. You can dock and undock, add or remove tabs as
needed. However, you cannot add or remove tabs from the properties panel.

== Overview

The overview tab provides statistics and general information about the current file opened, including:

- Amount of vehicles.
- Amount of services.
- Amount of stations.
- Amount of intervals.

== Searching

Each table would feature a search bar at the top. You can use it to filter the table entries based on your input.

The search bar currently supports matching Pinyin, Double Pinyin (Microsoft scheme) and Romaji for Chinese and Japanese
text.

== Diagnostics

The diagnostics tab provides information about potential issues in the current file opened. You can also access the tab
from the #link(<status-bar>)[status bar]

== Status Bar <status-bar>

The status bar is located at the bottom of the window. It provides quick access to various functions and information,
including:

- Tooltips.
- Diagnostics.

== Vehicle

The vehicle view tab is opened upon selecting a vehicle from the main canvas. You can edit the vehicle's timetable
entries, services, stops, and other properties here.

== All Vehicles

== All Services

== Global Search

The global search window allows you to search for various objects in the current file opened. You can use the
#kbd[Ctrl][Shift][F] shortcut to open the global search window.

== Station

== Route

== Route Diagram

== Shortcuts

Shortcuts are what makes power users. Here are all shortcuts listed out:

#figure(
  caption: [Keyboard Shortcuts],
  table(
    columns: (1fr, auto),
    align: start + horizon,
    table.header[Action][Shortcut],
    [Close tab], kbd[Ctrl][W],
    [Close app], kbd[Alt][F4],
    [Open file], kbd[Ctrl][O],
    [Save file], kbd[Ctrl][S],
    [Save file as], kbd[Ctrl][Shift][S],
    [Undo], kbd[Ctrl][Z],
    [Redo], kbd[Ctrl][Y],
    [Cut], kbd[Ctrl][X],
    [Copy], kbd[Ctrl][C],
    [Paste], kbd[Ctrl][V],
    [Select all], kbd[Ctrl][A],
    [Find], kbd[Ctrl][F],
    [Find next], kbd[F3],
    [Find previous], kbd[Shift][F3],
    [Global search], kbd[Ctrl][Shift][F],
  ),
)

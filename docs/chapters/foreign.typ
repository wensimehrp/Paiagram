= Importing Foreign Files <wip>

Paiagram supports importing diagram data from foreign formats including:

- qETRC
- OuDiaSecond

Data specific to one application, e.g. window placement settings in OuDiaSecond, will not be respected.

== qETRC

Importing qETRC data is straightforward: import and it should just work. Train classes, services, trains (交路), stations, and station intervals are all handled correctly in Paiagram.

== OuDiaSecond

OuDiaSecond's internal structure is very different, and it misses some basic features in Paiagram.

- Stations

  Importing stations is fine

- Intervals

  OuDiaSecond provides minimal information about intervals connecting stations, only the stations the interval connects. Due to this lack of information, intervals imported would always have a length of 1.00km.

- Services

  Most services should work fine.


== OpenTTD JGRPP Orders Exports

Importing JGRPP orders exports is fairly limited, since it could contain _conditional_ orders that cannot be known by Paiagram. The only condition Paiagram understands and can adapt from the export is the time.

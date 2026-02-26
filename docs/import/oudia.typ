#import "../book.typ": book-page

#show: book-page.with(title: "Importing OuDia and OuDiaSecond Files")

#link("https://web.archive.org/web/20250831042417/http://take-okm.a.la9.jp/oudia/")[OuDia]
and #link("http://oudiasecond.seesaa.net/")[OuDiaSecond] are popular Japanese timetabling applications. Paiagram
provides builtin support for reading their output formats, `.oud` and `.oud2`.

You can also checkout the #link("https://wensimehrp.github.io/oudia-to-kdl/")[oud-to-kdl] converter to convert both file
formats to #link("https://kdl.dev/")[KDL] in case if you want to process the `oud` format using a custom script.

= Importing in the App

You can import OuDia and OuDiaSecond files via the "More..." button on the top left-hand corner. Select "Import
OuDia..." for OuDia files, and select "Import OuDiaSecond..." for OuDiaSecond files.

= Importing with Command Line Arguments

You can also import both file formats via the `-o` or `--open` command line arguments. Simply follow your filename by
`-o` or `--open`, then launch the application. You should see your file(s) being imported.

= Important Notes

When importing OuDia files, Paiagram would assume that the file is Shift-JIS encoded. When importing OuDiaSecond files,
Paiagram would assume the file is UTF-8 encoded.

For both OuDia and OuDiaSecond formats, Paiagram would try to merge stations based on thet *station name* when
importing. For OuDiaSecond formats, Paiagram would also look at the station's connectivity info, including the loop-line
station and branched line settings.

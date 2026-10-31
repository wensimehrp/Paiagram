#title[Route Timetable]

A route timetable shows the times of trips running on a specific route. The first column is all
stations in the route, and each column after the first column lists a trip travelling on the route.
Each row displays when a trip arrives or depart from a station, or the platform/track the trip will
be staying at. The route timetable tile in Paiagram is based on Japanse train route timetables (e.g.
the table in https://but.tw/font/).

The table includes these elements:

#html.div({
  import "@preview/based:0.2.0"
  let font-bytes = read("/crates/paiagram/assets/fonts/Paiagram-Timetable-Font.ttf", encoding: none)
  let font-base64 = "data:font/truetype;charset=utf-8;base64," + based.base64.encode(font-bytes)
  let css-block = ```css
  @font-face {
      font-family: "Paiagram-Timetable-Font";
      src: url("__SOURCE__") format("truetype");
      font-style: normal;
      font-weight: normal;
      text-rendering: optimizeLegibility;
  }
  .ptf {
      font-family: "Paiagram-Timetable-Font";
      user-select: none;
  }
  ```
  html.style(
    css-block.text.replace("__SOURCE__", font-base64),
  )
  table(
    columns: 2,
    html.span(class: "ptf")[⇂],
    [Non-stop. This trip passes this station but does not stop at this station],

    html.span(class: "ptf")[||],
    [Does not pass station. This trip travels somewhere else and does not pass nor stop at this
      station],

    html.span(class: "ptf")[..],
    [Not in operation. Trip hasn't started its operation or has already ended its operation by this
      station and timepoint],

    html.span(class: "ptf")[1234],
    [Time, 12:34. Trip arrives at or departs from this station at this time.],

    html.span(class: "ptf")[936],
    [Time, 9:36. Trip arrives at or departs from this station at this time.],

    html.span(class: "ptf")[=====],
    [Ending mark. Trip ends its operation before or at this station],
  )
})

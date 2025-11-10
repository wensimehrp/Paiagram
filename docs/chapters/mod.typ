#let kbd(..args, mix-color: blue) = context {
  if target() == "html" {
    return args.pos().map(it => html.kbd(it)).join()
  }
  let shortcuts = args
    .pos()
    .map(word => {
      box({
        let key = align(center + horizon, text(
          size: .7em,
          font: "Fira Sans",
          weight: "bold",
          word,
          fill: black.lighten(30%).mix((mix-color, 20%)),
        ))
        let width = calc.max(measure(key).width, .7em.to-absolute())
        place(dy: 1pt, box(
          stroke: 1pt + gray.darken(30%).mix((mix-color, 10%)),
          width: width,
          inset: .1em,
          outset: .1em,
          radius: 2pt,
          fill: gray.darken(30%).mix((mix-color, 10%)),
          hide(key),
        ))
        box(
          stroke: 1pt
            + gradient.linear(
              angle: 60deg,
              white.mix((mix-color, 10%)),
              gray.mix((mix-color, 10%)),
            ),
          width: width,
          inset: .1em,
          outset: .1em,
          radius: 2pt,
          fill: gradient.linear(
            gray.lighten(50%).mix((mix-color, 10%)),
            white.mix((mix-color, 10%)),
          ),
          key,
        )
      })
    })
  shortcuts.intersperse(h(.4em)).join()
}

#kbd[1][2][Ctrl]

#let path(..args, mix-color: yellow.darken(50%)) = context {
  if target() == "html" {
    return args.pos().map(it => html.kbd(class: "path", it)).join()
  }
  set text(size: .7em, font: "Fira Sans", weight: "bold", fill: gray.mix(mix-color).darken(50%))
  let paths = args
    .pos()
    .map(key => {
      box({
        place(
          dy: -.4em,
          box(
            stroke: mix-color.lighten(70%),
            fill: mix-color.lighten(70%),
            inset: .1em,
            outset: -.1em,
            hide(key),
          ),
        )
        place(
          dy: -.2em,
          box(
            stroke: mix-color,
            fill: mix-color,
            inset: .1em,
            hide(key),
          ),
        )
        box(
          stroke: gradient.linear(
            angle: 90deg,
            mix-color.lighten(70%),
            mix-color.lighten(30%),
          ),
          fill: gradient.linear(
            angle: -90deg,
            mix-color.lighten(70%),
            mix-color.lighten(50%),
          ),
          outset: .1em,
          inset: .1em,
          key,
        )
      })
    })
  paths.intersperse("  ").join()
}

#let ph(class, style: none, weight: "ph-bold") = context if target() == "html" {
  html.i(
    class: ("inline-flex", "-translate-y-[0.1em]", weight)
      + if style == none {
        let t = if type(class) == str { class } else if type(class) == content { class.text }
        if not t.starts-with("ph-") {
          t = "ph-" + t
        }
        (t,)
      } else {
        (style, class)
      },
    [],
  )
} else {
  // do nothing currently
}

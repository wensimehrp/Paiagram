#import html as h
#import "@preview/based:0.2.0"
#import "@preview/typhoon:0.2.0": tailwind-css, update-elem
#let font-bytes = read("build/Paiagram-Timetable-Font.woff2", encoding: none)
#let font-base64 = "data:font/woff2;charset=utf-8;base64," + based.base64.encode(font-bytes)

#let title-text = "Paiagram-Timetable-Font"

#show h.elem: update-elem

#h.html({
  h.head({
    h.meta(charset: "utf-8")
    h.meta(name: "viewport", content: "width=device-width, initial-scale=1")
    h.title(title-text)
    h.elem("script", attrs: (
      defer: "",
      src: "https://stats.paiagram.com/script.js",
      data-website-id: "958a845f-415f-459e-b5a5-7bae194ba9c2",
    ))
    context h.style({
      let css-block = ```css
      @import url('https://fonts.googleapis.com/css2?family=Lato:ital,wght@0,100;0,300;0,400;0,700;0,900;1,100;1,300;1,400;1,700;1,900&display=swap');
      @font-face {
          font-family: "Paiagram-Timetable-Font";
          src: url("__SOURCE__") format("woff2");
          font-style: normal;
          font-weight: normal;
          text-rendering: optimizeLegibility;
      }
      :root {
        font-family: "Lato";
      }
      .ptf {
          font-family: "Paiagram-Timetable-Font";
      }
      ```
      css-block.text.replace("__SOURCE__", font-base64)
      tailwind-css()
    })
  })
  h.body(class: "bg-white dark:bg-neutral-800")[
    #h.article(class: "prose prose-neutral dark:prose-invert px-5 mx-auto my-10")[
      #title(title-text)
      This is the preview of the font "#title-text".

      = Timetable

      #let stations = (
        "Vancouver",
        "Coquitlam",
        "Mission City",
        "North Bend",
        "Kamloops",
        "Salmon Arm",
        "Revelstoke",
        "Field",
      )
      #let current-t = datetime(hour: 12, minute: 0, second: 0)
      #h.table.with(class: "ptf w-min mx-auto")({
        h.thead(class: "font-sans", {
          h.th(class: "min-w-30")[Station]
          h.th[Arr.]
          h.th[Dept.]
        })
        h.tbody({
          let tr = h.tr.with(class: "p-0")
          let td = h.td.with(class: "py-0 text-center")
          for (i, stn) in stations.enumerate() {
            tr({
              h.th(scope: "row", class: "font-normal", stn)
              current-t += duration(minutes: 25, seconds: 37)
              let arr = current-t
              current-t += duration(minutes: 2, seconds: 4)
              let dep = current-t
              for time in (arr, dep) {
                td({
                  time.display("[hour]:[minute]")
                  h.small(class: "pl-[1.5pt]", {
                    h.span(class: "sr-only")[:]
                    time.display("[second]")
                  })
                })
              }
            })
          }
          tr({
            h.th[]
            td[======]
            td[======]
          })
        })
      })

      = Glyphs

      #h.textarea(
        id: "input-textarea",
        placeholder: "Type your message to preview the font...",
        class: "w-full border ptf px-2 py-1 h-50 border-neutral-500/50 bg-neutral-500/10",
      )

      #let common-font-sizes = (6, 9, 11, 14, 18, 24, 36).rev()
      #h.p(class: "grid grid-cols-[auto_1fr] gap-3", for size in common-font-sizes {
        let size-str = str(size) + "pt"
        h.span(size-str)
        h.span(class: "text-mirror ptf my-0 text-[" + size-str + "]")
      })

      // Actually, this is just Basic JavaScript Jessie...
      #h.script(
        ```js
        const source = document.querySelector('#input-textarea');
        const targets = document.querySelectorAll('.text-mirror');

        source.addEventListener('input', () => {
          targets.forEach(target => {
            target.textContent = source.value;
          });
        });
        ```.text,
      )

      = About the Font

      #import "@preview/cmarker:0.1.10"
      #cmarker.render(read("README.md"))

      #h.div(class: "text-center", link(<index>)[Return to home page])
    ]
  ]
})

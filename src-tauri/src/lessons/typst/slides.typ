// oStudy lesson slides template.
//
// The native side writes the lesson's slides to /data.json and compiles this
// file; each slide is exactly one 16:9 page. Every learner- or model-authored
// string arrives as JSON data and is placed as text, never evaluated as
// markup. The only evaluated code is math translated from the bounded LaTeX
// subset (`m` runs). Colours and typefaces are the study guide themes.
#import "themes.typ": themes

#let data = json("/data.json")
#let T = themes.at(data.theme)
#let head = T.head
#let ui = T.ui
#let W = 254mm
#let H = 142.875mm
#let MX = 16mm

#set document(title: data.title)
#set page(width: W, height: H, margin: 0mm, fill: T.paper)
#set text(font: (ui, "IBM Plex Sans"), size: 15pt, fill: T.ink, lang: "en", hyphenate: false)
#set par(justify: false, leading: 0.55em, spacing: 0.7em)
#show math.equation: set text(font: ("New Computer Modern Math",))

#let rt(runs) = {
  for run in runs {
    if "t" in run {
      run.t
    } else if "m" in run {
      math.equation(block: run.block, eval(run.m, mode: "math"))
    } else if "src" in run {
      text(font: ("DejaVu Sans Mono",), size: 0.9em, run.src)
    }
  }
}

#let eyebrow(body, color: T.accent, s: 1) = text(font: ui, size: 9.5pt * s, weight: 700, tracking: 0.14em, fill: color, upper(body))

// Lays out `content(s)` at the largest scale `s` whose height fits the space:
// a sparse slide grows to fill it, and a long one shrinks its type instead of
// running onto a second page.
#let fitted(content) = layout(size => {
  let scales = (1.6, 1.45, 1.32, 1.2, 1.1, 1, 0.92, 0.85, 0.78, 0.71, 0.64, 0.58, 0.52)
  let chosen = scales.last()
  for s in scales {
    if measure(block(width: size.width, content(s))).height <= size.height {
      chosen = s
      break
    }
  }
  block(width: size.width, height: size.height, clip: true, content(chosen))
})

// The slim band every content slide opens with: the guide's chapter colours.
#let band = place(top + left, grid(columns: (62%, 22%, 16%),
  rect(width: 100%, height: 2.4mm, fill: T.primary),
  rect(width: 100%, height: 2.4mm, fill: T.accent),
  rect(width: 100%, height: 2.4mm, fill: T.highlight)))

#let footer(slide) = place(bottom + left, dx: MX, dy: -6mm, block(width: W - 2 * MX)[
  #line(length: 100%, stroke: 0.6pt + T.rule)
  #v(-1mm)
  #grid(columns: (1fr, auto), align: (left + horizon, right + horizon),
    text(font: ui, size: 8.5pt, fill: T.muted, data.title),
    text(font: ui, size: 8.5pt, weight: 700, fill: T.muted, str(slide.number)))])

#let points-view(points, s) = grid(columns: (6mm * s, 1fr), row-gutter: 3.2mm * s,
  ..points.map(p => (
    text(size: 15pt * s, weight: 700, fill: T.accent, "—"),
    text(size: 15pt * s, rt(p)))).flatten())

#let callout-style(kind) = if kind == "warning" {
  (fill: T.warn-soft, color: T.warn, label: "Watch out")
} else if kind == "tip" {
  (fill: T.good-soft, color: T.good, label: "Tip")
} else if kind == "note" {
  (fill: T.accent-soft, color: T.accent, label: "Note")
} else {
  (fill: T.highlight-soft, color: T.highlight-ink, label: "Key idea")
}

#let tile(fill, s, body) = block(width: 100%, fill: fill, radius: 2.4mm, inset: (x: 5mm * s, y: 4mm * s), body)

#let flow-view(vis, s) = {
  let n = vis.steps.len()
  let per-row = if n <= 5 { n } else { calc.ceil(n / 2) }
  let rows = range(0, n, step: per-row).map(i => vis.steps.slice(i, calc.min(n, i + per-row)))
  stack(spacing: 4mm * s, ..rows.enumerate().map(((ri, row)) => {
    let last-row = ri == rows.len() - 1
    grid(columns: ((1fr, 7mm * s) * per-row).slice(0, 2 * per-row - 1), align: horizon,
      ..row.enumerate().map(((i, st)) => {
        let index = ri * per-row + i
        let final = last-row and i == row.len() - 1
        let card = block(width: 100%, fill: if final { T.primary } else { T.accent-soft }, radius: 2.4mm, inset: (x: 3.5mm * s, y: 3.5mm * s))[
          #text(font: head, size: 15pt * s, weight: 700, fill: if final { T.highlight } else { T.accent }, str(index + 1))
          #h(1.5mm * s)
          #text(size: 12.5pt * s, weight: 600, fill: if final { white } else { T.primary }, rt(st))]
        if i < per-row - 1 { (card, align(center, text(font: ui, size: 16pt * s, weight: 700, fill: T.accent, if i < row.len() - 1 { "›" } else { "" }))) } else { (card,) }
      }).flatten())
  }))
}

#let visual-view(vis, s) = {
  let kind = vis.kind
  if kind == "flow" {
    if vis.title.len() > 0 { eyebrow(vis.title.map(r => r.at("t", default: "")).join(), color: T.muted, s: s); v(2mm * s) }
    flow-view(vis, s)
  } else if kind == "compare" {
    grid(columns: (1fr, 1fr), column-gutter: 5mm * s,
      block(width: 100%, fill: T.primary, radius: 2.4mm, inset: 5mm * s, text(size: 15pt * s, weight: 600, fill: white, rt(vis.a))),
      block(width: 100%, fill: T.accent, radius: 2.4mm, inset: 5mm * s, text(size: 15pt * s, weight: 600, fill: white, rt(vis.b))))
    v(3mm * s)
    tile(T.accent-soft, s)[#eyebrow("How they relate", s: s) \ #text(size: 13.5pt * s, rt(vis.relationship))]
  } else if kind == "table" {
    if vis.title.len() > 0 { text(size: 12pt * s, weight: 700, fill: T.primary, rt(vis.title)); v(1.5mm * s) }
    table(columns: (1fr,) * calc.max(1, vis.columns.len()), stroke: none, inset: (x: 3.5mm * s, y: 2.6mm * s),
      fill: (x, y) => if y == 0 { T.primary } else if calc.even(y) { T.accent-soft } else { none },
      table.header(..vis.columns.map(c => text(size: 11pt * s, weight: 700, fill: white, rt(c)))),
      ..vis.rows.join().map(c => text(size: 11.5pt * s, rt(c))))
    if vis.caption.len() > 0 { v(1mm * s); text(size: 9.5pt * s, fill: T.muted, rt(vis.caption)) }
  } else if kind == "term" {
    block(width: 100%, stroke: (left: 2mm * s + T.accent), inset: (left: 6mm * s, y: 2mm * s))[
      #text(font: head, size: 26pt * s, weight: 700, fill: T.primary, rt(vis.term))
      #v(2mm * s)
      #text(size: 15pt * s, rt(vis.explanation))]
  } else if kind == "myth" {
    grid(columns: (1fr, 1fr), column-gutter: 5mm * s,
      tile(T.warn-soft, s)[#eyebrow("Common mistake", color: T.warn, s: s) \ #text(size: 14pt * s, rt(vis.myth))],
      tile(T.good-soft, s)[#eyebrow("Actually", color: T.good, s: s) \ #text(size: 14pt * s, rt(vis.fact))])
  } else if kind == "callout" {
    let style = callout-style(vis.callout)
    block(width: 100%, fill: style.fill, radius: (right: 2.4mm), stroke: (left: 1.6mm * s + style.color), inset: (x: 5mm * s, y: 4.5mm * s))[
      #eyebrow(style.label, color: style.color, s: s) \
      #text(font: head, size: 16pt * s, weight: 700, fill: T.primary, rt(vis.title))
      #v(1mm * s)
      #text(size: 13.5pt * s, rt(vis.body))]
  } else if kind == "example" {
    tile(T.good-soft, s)[
      #eyebrow("Worked example", color: T.good, s: s) #h(2mm) #text(size: 13pt * s, weight: 700, fill: T.primary, rt(vis.title)) \
      #text(size: 13pt * s, style: "italic", rt(vis.prompt))
      #v(1.5mm * s)
      #for (i, st) in vis.steps.enumerate() [#text(size: 12.5pt * s, weight: 700, fill: T.good, str(i + 1) + ".") #text(size: 12.5pt * s, rt(st)) \ ]
      #v(1mm * s)
      #text(size: 13pt * s, weight: 700, fill: T.good, rt(vis.answer))]
  }
}

// ---------- Slides ----------
#let title-slide(slide) = page(fill: T.primary)[
  #place(top + right, dx: 30mm, dy: -40mm, circle(radius: 62mm, fill: white.transparentize(94%)))
  #for (i, r) in (46, 34, 22).enumerate() {
    place(top + right, dx: 30mm - (62mm - r * 1mm), dy: -40mm + (62mm - r * 1mm), circle(radius: r * 1mm, stroke: 1.2pt + T.band-light.transparentize(40% + i * 15%)))
  }
  #place(bottom + right, dx: -26mm, dy: 20mm, circle(radius: 26mm, fill: T.highlight))
  #place(top + left, dx: MX, dy: 18mm, block(width: W - 2 * MX - 50mm, height: H - 36mm, fitted(s => [
    #eyebrow(slide.eyebrow, color: T.band-light, s: s)
    #v(4mm * s)
    #text(font: head, size: 34pt * s, weight: 700, fill: white, rt(slide.title))
    #if slide.points.len() > 0 [
      #v(7mm * s)
      #grid(columns: (7mm * s, 1fr), row-gutter: 2.6mm * s,
        ..slide.points.enumerate().map(((i, p)) => (
          text(size: 13pt * s, weight: 700, fill: T.highlight, str(i + 1)),
          text(size: 13pt * s, fill: white.transparentize(10%), rt(p)))).flatten())
    ]
  ])))
  #place(bottom + left, dx: MX, dy: -9mm, text(font: ui, size: 9pt, fill: T.band-light, slide.meta))
]

#let content-slide(slide) = page[
  #band
  #place(top + left, dx: MX, dy: 11mm, block(width: W - 2 * MX, height: H - 11mm - 17mm, fitted(s => {
    eyebrow(slide.eyebrow, s: s)
    v(1.5mm * s)
    block(text(font: head, size: 25pt * s, weight: 700, fill: T.primary, rt(slide.title)))
    v(5mm * s)
    let vis = slide.visual
    let fig = slide.at("figure", default: none)
    if fig != none {
      // A figure from the source sits beside the points, or under them
      // when it is wide; its words stay with it.
      let picture(height) = figure(
        block(height: height, width: 100%, image(fig.path, fit: "contain", width: 100%, height: 100%)),
        caption: text(size: 9.5pt * s, fill: T.muted, rt(fig.caption)),
        numbering: none, gap: 2mm * s)
      if slide.points.len() == 0 {
        picture(88mm * s)
      } else if fig.tall {
        grid(columns: (1fr, 1fr), column-gutter: 8mm * s, points-view(slide.points, s * 0.95), picture(82mm * s))
      } else {
        grid(columns: (1.1fr, 1fr), column-gutter: 8mm * s, points-view(slide.points, s * 0.95), picture(64mm * s))
      }
    } else if vis == none {
      points-view(slide.points, s * 1.08)
    } else if vis.kind == "flow" or vis.kind == "table" or vis.kind == "compare" or vis.kind == "myth" {
      if slide.points.len() > 0 { points-view(slide.points, s * 0.92); v(5mm * s) }
      visual-view(vis, s)
    } else {
      grid(columns: if slide.points.len() > 0 { (1fr, 1fr) } else { (1fr,) }, column-gutter: 8mm * s,
        ..if slide.points.len() > 0 { (points-view(slide.points, s * 0.95),) } else { () },
        visual-view(vis, s))
    }
  })))
  #footer(slide)
]

#let sources-slide(slide) = page[
  #band
  #place(top + left, dx: MX, dy: 11mm, block(width: W - 2 * MX, height: H - 11mm - 17mm, fitted(s => [
    #eyebrow(slide.eyebrow, s: s)
    #v(1.5mm * s)
    #text(font: head, size: 25pt * s, weight: 700, fill: T.primary, rt(slide.title))
    #v(6mm * s)
    #grid(columns: (1fr, 1fr), column-gutter: 5mm * s, row-gutter: 4mm * s,
      ..slide.points.map(p => tile(T.accent-soft, s, text(size: 13pt * s, weight: 600, fill: T.primary, rt(p)))))
  ])))
  #footer(slide)
]

#for slide in data.slides {
  if slide.kind == "title" { title-slide(slide) } else if slide.kind == "sources" { sources-slide(slide) } else { content-slide(slide) }
}

// Building blocks shared by the study guide PDF and the live section pages.
//
// Both templates write their data, with its `theme` key, to /data.json; every
// learner- or model-authored string is placed as text, never evaluated as
// markup. The only evaluated code is math translated from the exporter's
// bounded LaTeX subset (`m` runs).
#import "themes.typ": themes

#let data = json("/data.json")
#let T = themes.at(data.theme)
#let paper = T.paper
#let head = T.head
#let ui = T.ui

#let eyebrow(body, color: T.accent) = text(font: ui, size: 7pt, weight: 700, tracking: 0.12em, fill: color, upper(body))
#let hairline = line(length: 100%, stroke: 0.5pt + T.rule)

// Rich text: a list of runs, each plain text (t), translated math (m, with
// block) or LaTeX kept as source (src) when it is outside the native subset.
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
#let plain(runs) = runs.map(run => run.at("t", default: run.at("src", default: ""))).join()

// ---------- Building blocks ----------
#let tile(body, fill: T.accent-soft, inset: (x: 5mm, y: 4mm)) = block(width: 100%, fill: fill, radius: 2mm, inset: inset, above: 4mm, below: 4mm, breakable: false, body)
#let bullets(items) = pad(left: 1mm, grid(columns: (4.5mm, 1fr), row-gutter: 1.6mm,
  ..items.map(item => (text(font: ui, fill: T.accent, weight: 700, "—"), rt(item))).flatten()))
#let subheading(body) = block(above: 4mm, below: 2mm, sticky: true, text(font: ui, size: 8.8pt, weight: 700, fill: T.primary, rt(body)))

#let callout-style(label) = if label == "Watch out" {
  (fill: T.warn-soft, color: T.warn)
} else if label == "Study tip" or label == "Note" {
  (fill: T.good-soft, color: T.good)
} else {
  (fill: T.accent-soft, color: T.accent)
}

#let block-view(b) = {
  let kind = b.kind
  if kind == "lead" {
    par(justify: false, text(size: T.body-size * 1.12, rt(b.text)))
  } else if kind == "paragraph" {
    block(rt(b.text))
  } else if kind == "heading" {
    subheading(b.text)
  } else if kind == "bullets" {
    bullets(b.items)
  } else if kind == "numbered" {
    if b.items.len() >= 2 and b.items.len() <= 4 {
      let n = b.items.len()
      block(above: 2mm, below: 3mm, grid(columns: (1fr, 5mm) * (n - 1) + (1fr,), align: horizon,
        ..b.items.enumerate().map(((i, st)) => {
          let card = block(width: 100%, fill: T.accent-soft, radius: 2mm, inset: 3mm, breakable: false)[
            #set par(justify: false)
            #text(font: head, size: 13pt, weight: 700, fill: T.accent, str(i + 1)) \ #text(size: 0.88em, rt(st))]
          if i < n - 1 { (card, align(center, text(font: ui, size: 13pt, weight: 700, fill: T.accent, "›"))) } else { (card,) }
        }).flatten()))
    } else {
      pad(left: 1mm, grid(columns: (6mm, 1fr), row-gutter: 1.8mm,
        ..b.items.enumerate().map(((i, st)) => (text(font: ui, weight: 700, fill: T.accent, str(i + 1)), rt(st))).flatten()))
    }
  } else if kind == "terms" {
    tile(fill: T.accent-soft, inset: 4mm)[
      #eyebrow("Terms") #v(1mm)
      #set par(justify: false)
      #grid(columns: (1fr, 1fr), column-gutter: 6mm, row-gutter: 2.5mm,
        ..b.terms.map(((term, meaning)) => [#text(font: ui, size: 0.85em, weight: 700, fill: T.primary, rt(term)) \ #text(size: 0.92em, rt(meaning))]))]
  } else if kind == "callout" {
    let style = callout-style(b.label)
    tile(fill: style.fill)[
      #eyebrow(b.label, color: style.color) #h(2mm) #text(font: ui, size: 10pt, weight: 700, fill: T.primary, rt(b.title)) \
      #set par(justify: false)
      #rt(b.body)]
  } else if kind == "table" {
    block(above: 3mm, below: 3mm)[
      #if b.title.len() > 0 [#text(font: ui, size: 8.6pt, weight: 700, fill: T.primary, rt(b.title)) #v(0.5mm)]
      #set par(justify: false)
      #table(columns: (1fr,) * calc.max(1, b.columns.len()), stroke: none, inset: (x: 3mm, y: 2.2mm),
        fill: (x, y) => if y == 0 { T.primary } else if calc.even(y) { T.accent-soft } else { none },
        table.header(..b.columns.map(c => text(font: ui, size: 8pt, weight: 700, fill: white, rt(c)))),
        ..b.rows.join().map(c => text(font: ui, size: 8.6pt, rt(c))))
      #if b.caption.len() > 0 [#text(font: ui, size: 7.4pt, fill: T.muted, rt(b.caption))]]
  } else if kind == "example" {
    tile(fill: T.good-soft)[
      #eyebrow("Worked example", color: T.good) #h(2mm) #text(font: ui, size: 10pt, weight: 700, fill: T.primary, rt(b.title)) \
      #set par(justify: false)
      #emph(rt(b.prompt))
      #v(1mm)
      #for (i, st) in b.steps.enumerate() [#text(font: ui, size: 8pt, weight: 700, fill: T.good, str(i + 1) + ".") #rt(st) \ ]
      #v(0.5mm)
      #text(weight: 700, fill: T.good, rt(b.answer)) #text(size: 0.92em, fill: T.muted, rt(b.takeaway))]
  } else if kind == "check" {
    tile(fill: T.highlight-soft, inset: (x: 5mm, y: 3.5mm))[
      #set par(justify: false)
      #grid(columns: (1fr, 1fr), gutter: 6mm,
        [#eyebrow("Check yourself", color: T.highlight-ink) \ #text(weight: 600, rt(b.prompt))],
        [#eyebrow("Answer", color: T.muted) \ #text(fill: T.muted, rt(b.answer))])]
  } else if kind == "spacer" {
    v(2mm)
  }
}

#let figure-view(f, n) = block(above: 4mm, below: 4mm, breakable: false)[
  #align(center, image(f.path, width: 100%, height: auto, fit: "contain"))
  #v(1.5mm)
  #set par(justify: false)
  #text(font: ui, size: 7.6pt, fill: T.muted)[#text(weight: 700, fill: T.primary, "Figure " + n + "  ") #rt(f.title)#if f.caption.len() > 0 [. #rt(f.caption)]]]

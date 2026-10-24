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

// Rich text: a list of runs, each plain text (t), a bold key term (b), an
// organism's italic name (i), a formula's subscript (sub) or superscript
// (sup), translated math (m, with block) or LaTeX kept as
// source (src) when it is outside the native subset.
#let rt(runs) = {
  for run in runs {
    if "t" in run {
      run.t
    } else if "b" in run {
      text(weight: 700, fill: T.primary, run.b)
    } else if "i" in run {
      emph(run.i)
    } else if "sub" in run {
      sub(run.sub)
    } else if "sup" in run {
      super(run.sup)
    } else if "m" in run {
      math.equation(block: run.block, eval(run.m, mode: "math"))
    } else if "src" in run {
      text(font: ("DejaVu Sans Mono",), size: 0.9em, run.src)
    }
  }
}
#let plain(runs) = runs.map(run => run.at("t", default: run.at("b", default: run.at("i", default: run.at("sub", default: run.at("sup", default: run.at("src", default: ""))))))).join()

// ---------- Building blocks ----------
#let tile(body, fill: T.accent-soft, inset: (x: 4.5mm, y: 3mm)) = block(width: 100%, fill: fill, radius: 2mm, inset: inset, above: 2.6mm, below: 2.6mm, breakable: false, body)
#let bullets(items) = pad(left: 1mm, grid(columns: (4.5mm, 1fr), row-gutter: 1.2mm,
  ..items.map(item => (text(font: ui, fill: T.accent, weight: 700, "—"), rt(item))).flatten()))
#let subheading(body) = block(above: 3mm, below: 1.5mm, sticky: true, text(font: ui, size: 8.8pt, weight: 700, fill: T.primary, rt(body)))

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
    // Beside a figure the column is narrow, so the answer goes under the
    // question rather than beside it.
    tile(fill: T.highlight-soft, inset: (x: 5mm, y: 3.5mm), layout(size => {
      set par(justify: false)
      let question = [#eyebrow("Check yourself", color: T.highlight-ink) \ #text(weight: 600, rt(b.prompt))]
      let answer = [#eyebrow("Answer", color: T.muted) \ #text(fill: T.muted, rt(b.answer))]
      if size.width < 120mm { stack(spacing: 2.5mm, question, answer) } else { grid(columns: (1fr, 1fr), gutter: 6mm, question, answer) }
    }))
  } else if kind == "spacer" {
    v(2mm)
  }
}

#let figure-caption(f, n) = [
  #set par(justify: false)
  #text(font: ui, size: 7.6pt, fill: T.muted)[#text(weight: 700, fill: T.primary, "Figure " + n + "  ") #rt(f.title)#if f.caption.len() > 0 [. #rt(f.caption)]]]

#let figure-body(f, n, width: auto) = [
  #align(center, image(f.path, width: if width == auto { f.width * 1mm } else { width }, height: auto, fit: "contain"))
  #v(1.5mm)
  #figure-caption(f, n)]

// A figure no wider than this sits beside the text around it; a wider one
// keeps the full width.
#let float-max-width = 112
#let floats(f) = f.width <= float-max-width

// A wide figure, or two narrow ones in a row, floats to the top or foot of
// the page it fits on, so the text after it fills the rest of the page
// instead of leaving it empty.
#let floated(body) = place(auto, float: true, clearance: 5mm, block(width: 100%, breakable: false, body))

// A narrow figure with no text left to sit beside takes the left of the row
// and its caption the rest.
#let figure-row(f, n) = block(above: 4mm, below: 4mm, breakable: false, width: 100%,
  grid(columns: (f.width * 1mm, 1fr), column-gutter: 6mm, align: (left + top, left + bottom),
    image(f.path, width: f.width * 1mm, height: auto, fit: "contain"), figure-caption(f, n)))

// Two narrow figures share a row when no text is left to wrap them.
#let pairs(figures) = figures.len() == 2 and figures.all(f => f.width <= 84)

// The narrowest a figure beside the text is drawn to match its height.
#let float-min-width = 55mm

// Text units a figure can sit beside: one key point, or one whole block.
#let units-of(blocks) = blocks.map(b => if b.kind == "bullets" {
  b.items.map(item => (kind: "point", item: item))
} else {
  ((kind: "block", block: b),)
}).flatten()

// Draws units in order, consecutive key points as one list.
#let units-view(units) = {
  let points = ()
  for u in units {
    if u.kind == "point" {
      points.push(u.item)
    } else {
      if points.len() > 0 { bullets(points); points = () }
      block-view(u.block)
    }
  }
  if points.len() > 0 { bullets(points) }
}

// A unit that may sit in the narrow column beside a figure: tables and
// term grids need the full width.
#let wraps(u) = u.kind == "point" or u.block.kind in ("lead", "paragraph", "heading", "check", "callout", "spacer")

// Sets the text units after a section's figures position around those
// figures. A narrow figure sits at the right of as many of the following
// units as reach its height, drawn smaller when the text is shorter, and the
// rest run full width; a wide figure, or a narrow one with no text left,
// floats so the page fills with text. `first` numbers the figures.
#let flow(units, figures, number, first: 1) = context {
  // Guide pages have one column of text this wide (A4 less 16 mm margins).
  let size = (width: 178mm)
  let gutter = 5mm
  let rest = units
  let figs = figures
  let index = first
  while figs.len() > 0 {
    let f = figs.first()
    let n = number + "." + str(index)
    let wrappable = rest.len() > 0 and wraps(rest.first())
    if not floats(f) {
      floated(figure-body(f, n))
      figs = figs.slice(1)
      index += 1
    } else if not wrappable {
      if figs.len() >= 2 and floats(figs.at(1)) {
        // Two narrow figures and no text left: they share a row.
        let cell(g, m) = block(width: 100%, figure-body(g, m, width: calc.min(g.width * 1mm, (size.width - 6mm) / 2)))
        floated(grid(columns: (1fr, 1fr), column-gutter: 6mm,
          cell(figs.at(0), n), cell(figs.at(1), number + "." + str(index + 1))))
        figs = figs.slice(2)
        index += 2
      } else {
        floated(figure-row(f, n))
        figs = figs.slice(1)
        index += 1
      }
    } else {
      let side-units = ()
      for u in rest { if wraps(u) { side-units.push(u) } else { break } }
      let text-at(width, us) = measure(block(width: size.width - width - gutter, units-view(us))).height
      // A second narrow figure stacks under the first, rather than being left
      // with no text of its own.
      let group = if figs.len() >= 2 and floats(figs.at(1)) { figs.slice(0, 2) } else { (f,) }
      let side(width, g) = stack(spacing: 4mm, ..g.enumerate().map(((i, h)) =>
        figure-body(h, number + "." + str(index + i), width: calc.min(width, h.width * 1mm))))
      let height-at(width, g) = measure(block(width: width, side(width, g))).height
      let widest(g) = calc.min(calc.max(..g.map(h => h.width)) * 1mm, size.width * 0.42)
      // Stacked, the pair may run somewhat past the text; that leaves less
      // empty paper than the second figure on a row of its own.
      if group.len() == 2 and height-at(float-min-width, group) > text-at(float-min-width, side-units) * 3 + 4mm {
        group = (f,)
      }
      let width = widest(group)
      while width > float-min-width and height-at(width, group) > text-at(width, side-units) + 4mm {
        width = calc.max(float-min-width, width - 4mm)
      }
      let tall = height-at(width, group)
      let column = size.width - width - gutter
      let count = 0
      let used = 0pt
      while count < side-units.len() and used < tall {
        used += measure(block(width: column, units-view((side-units.at(count),)))).height + 1.2mm
        count += 1
      }
      // A heading never ends the column; it goes with the text it heads.
      while count > 0 and count < rest.len() and side-units.at(count - 1).kind == "block" and side-units.at(count - 1).block.kind == "heading" {
        count -= 1
      }
      if count == 0 {
        floated(figure-row(f, n))
        group = (f,)
      } else {
        block(above: 2mm, below: 3mm, breakable: false, grid(columns: (1fr, width), column-gutter: gutter,
          units-view(rest.slice(0, count)), side(width, group)))
        rest = rest.slice(count)
      }
      figs = figs.slice(group.len())
      index += group.len()
    }
  }
  units-view(rest)
}

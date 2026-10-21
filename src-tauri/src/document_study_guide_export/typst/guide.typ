// oStudy study guide PDF template.
//
// The native exporter writes the effective guide to /data.json and compiles
// this file. Every learner- or model-authored string arrives as JSON data and
// is placed as text, never evaluated as markup. The only evaluated code is
// math the exporter translated from its bounded LaTeX subset (`m` runs).
#import "themes.typ": themes

#let data = json("/data.json")
#let T = themes.at(data.theme)
#let paper = T.paper
#let head = T.head
#let ui = T.ui

#set document(title: data.title)
#set text(font: (T.body, "Libertinus Serif"), size: T.body-size, fill: T.ink, lang: "en", hyphenate: true)
#set par(justify: true, leading: 0.64em, spacing: 0.85em)
#show math.equation: set text(font: ("New Computer Modern Math",))

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

// ---------- Cover ----------
#let has-chapters = data.chapters.len() > 0
#let contents-entries = if has-chapters {
  data.chapters.enumerate().map(((i, c)) => (label: "chapter-" + str(i + 1), number: str(i + 1), title: c.title))
} else {
  data.sections.enumerate().map(((i, s)) => (label: "section-" + str(i + 1), number: str(i + 1), title: s.title))
}
#let cover-limit = 12
#let page-of(name) = context {
  let found = query(label(name))
  if found.len() > 0 { str(counter(page).at(found.first().location()).first()) }
}
#let contents-line(entry, size: 8.8pt) = link(label(entry.label), grid(columns: (6mm, 1fr, auto),
  text(font: ui, size: size, weight: 700, fill: T.accent, entry.number),
  [#text(font: ui, size: size, rt(entry.title)) #box(width: 1fr, repeat(text(fill: T.muted.transparentize(50%), ".")))],
  text(font: ui, size: size, fill: T.muted, page-of(entry.label))))

#page(margin: (x: 22mm, top: 26mm, bottom: 18mm), fill: paper)[
  #place(top + left, dx: -22mm, dy: -26mm, rect(width: 210mm, height: 116mm, fill: T.primary))
  #place(top + right, dx: 22mm, dy: -26mm, block(width: 90mm, height: 116mm, clip: true, {
    for (i, r) in (60, 46, 32, 18).enumerate() {
      place(top + left, dx: 70mm - r * 1mm, dy: 40mm - r * 1mm, circle(radius: r * 1mm, stroke: 1.2pt + T.band-light.transparentize(40% + i * 10%)))
    }
    place(top + left, dx: 62mm, dy: 32mm, circle(radius: 8mm, fill: T.highlight))
  }))
  #eyebrow("ostudy · study guide", color: T.band-light)
  #v(34mm)
  #block(width: 120mm, height: 46mm)[
    #set par(justify: false, leading: 0.5em)
    #align(bottom)[
      #text(font: head, size: if data.title.len() > 48 { 26pt } else { 34pt }, weight: 700, fill: white, hyphenate: false, data.title)
      #v(1mm)
      #text(font: ui, size: 11pt, fill: white.transparentize(20%), "From " + data.source)]]
  #v(26mm)
  #grid(columns: (1fr, 66mm), gutter: 12mm,
    [
      #set par(justify: false)
      #text(size: T.body-size * 1.2, rt(data.overview))
      #if data.objectives.len() > 0 [
        #v(5mm)
        #eyebrow("You will be able to") #v(1mm)
        #for (i, o) in data.objectives.enumerate() [
          #grid(columns: (6mm, 1fr), text(font: ui, weight: 700, fill: T.accent, str(i + 1)), rt(o)) #v(0.6mm)]
      ]
    ],
    block(width: 100%, fill: T.accent-soft, radius: 2mm, inset: 5mm)[
      #eyebrow("Contents") #v(2mm)
      #set par(justify: false)
      #for entry in contents-entries.slice(0, calc.min(cover-limit, contents-entries.len())) [#contents-line(entry) #v(0.6mm)]
      #if contents-entries.len() > cover-limit [
        #text(font: ui, size: 8pt, fill: T.muted)[and #(contents-entries.len() - cover-limit) more on the next page]]
      #if data.keyIdeas.len() + data.checks.len() > 0 [
        #v(2mm)
        #if data.keyIdeas.len() > 0 [#contents-line((label: "cram", number: "", title: ((t: "Cram sheet"),))) #v(0.6mm)]
        #if data.checks.len() > 0 [#contents-line((label: "self-test", number: "", title: ((t: "Self-test"),)))]
      ]
    ])
  #v(1fr)
  #hairline
  #v(2mm)
  #text(font: ui, size: 7.5pt, fill: T.muted)[From #data.source · #data.sectionCount #if data.sectionCount == 1 [section] else [sections] · Made with oStudy]
]

// ---------- Running pages ----------
#set page(paper: "a4", fill: paper, margin: (top: 22mm, bottom: 18mm, x: 20mm),
  header: context [
    #set text(font: ui, size: 7pt, fill: T.muted)
    #text(weight: 600, fill: T.primary, data.title) #h(1fr) #data.source
    #v(-2mm) #hairline
  ],
  footer: context [#set text(font: ui, size: 7.5pt, fill: T.muted)
    #h(1fr) #counter(page).display() #h(1fr)])

#if contents-entries.len() > cover-limit [
  #text(font: head, size: 20pt, weight: 700, fill: T.primary, "Contents")
  #v(1mm) #line(length: 100%, stroke: 1.2pt + T.primary) #v(3mm)
  #set par(justify: false)
  #for entry in contents-entries [#contents-line(entry, size: 9.4pt) #v(1.2mm)]
  #pagebreak()
]

#if data.guideNotes.len() > 0 or data.mathNotice [
  #if data.guideNotes.len() > 0 [
    #tile(fill: T.highlight-soft)[#eyebrow("My guide notes", color: T.highlight-ink) #v(1mm) #bullets(data.guideNotes)]]
  #if data.mathNotice [#text(font: ui, size: 7.6pt, fill: T.muted, data.mathNoticeText)]
]

#let section-view(s, number, name) = {
  let h = heading(level: 2, outlined: true, bookmarked: true)[#text(font: head, size: 14pt, weight: 700, fill: T.accent, number) #h(1.4mm)#text(font: head, size: 14pt, weight: 700, fill: T.primary, rt(s.title))]
  block(above: 6mm, below: 3mm, sticky: true, grid(columns: (1fr, auto), column-gutter: 4mm, align: (left + bottom, right + bottom),
    if name != none [#h #label(name)] else { h },
    if s.source != none { text(font: ui, size: 7.5pt, fill: T.muted, s.source) }))
  for b in s.blocks { block-view(b) }
  for (i, f) in s.figures.enumerate() { figure-view(f, number + "." + str(i + 1)) }
  if s.notes.len() > 0 {
    tile(fill: T.highlight-soft)[#eyebrow("My notes", color: T.highlight-ink) #v(1mm) #bullets(s.notes)]
  }
}

#show heading.where(level: 2): it => it.body
#show heading.where(level: 1): it => place(hide(it.body))

#if has-chapters {
  for (ci, chapter) in data.chapters.enumerate() {
    block(width: 100%, fill: T.primary, radius: 2mm, inset: (x: 6mm, y: 4.5mm), above: 6mm, below: 4mm, breakable: false)[
      #heading(level: 1, outlined: true, bookmarked: true)[#plain(chapter.title)] #label("chapter-" + str(ci + 1))
      #grid(columns: (14mm, 1fr), align: (left + horizon, left + horizon),
        text(font: head, size: 28pt, weight: 700, fill: T.highlight, str(ci + 1)),
        [#eyebrow("Chapter " + str(ci + 1), color: T.band-light) \ #text(font: head, size: 16pt, weight: 700, fill: white, rt(chapter.title))
         #if chapter.intro.len() > 0 [ \
          #set par(justify: false)
          #text(size: 0.96em, fill: white.transparentize(15%), rt(chapter.intro))]])]
    for (pi, si) in chapter.sections.enumerate() {
      section-view(data.sections.at(si), str(ci + 1) + "." + str(pi + 1), none)
    }
  }
} else {
  for (si, s) in data.sections.enumerate() {
    section-view(s, str(si + 1), "section-" + str(si + 1))
  }
}

// ---------- Cram sheet ----------
#if data.keyIdeas.len() + data.terms.len() + data.keyConcepts.len() > 0 [
  #pagebreak()
  #heading(level: 1, outlined: true, bookmarked: true)[Cram sheet] #label("cram")
  #text(font: head, size: 22pt, weight: 700, fill: T.primary, "Cram sheet") #h(3mm) #text(font: ui, size: 9pt, fill: T.muted, "The whole guide on one page.")
  #v(1mm) #line(length: 100%, stroke: 1.2pt + T.primary)
  #v(3mm)
  #if data.keyConcepts.len() > 0 [
    #block(width: 100%, fill: T.primary, radius: 2mm, inset: (x: 5mm, y: 4mm))[
      #eyebrow("Key concepts", color: T.band-light) #v(1.5mm)
      #set par(justify: false)
      #grid(columns: (1fr, 1fr), column-gutter: 6mm, row-gutter: 1.6mm,
        ..data.keyConcepts.map(c => text(fill: white, [#text(fill: T.highlight, weight: 700, "■ ") #rt(c)])))]
    #v(3mm)
  ]
  #if data.keyIdeas.len() > 0 [
    #eyebrow("Key ideas") #v(1mm)
    #grid(columns: (1fr, 1fr), column-gutter: 3mm, row-gutter: 3mm,
      ..data.keyIdeas.map(k => block(width: 100%, fill: T.accent-soft, radius: 2mm, inset: 3.5mm, breakable: false)[
        #set par(justify: false)
        #text(font: ui, size: 9pt, weight: 700, fill: T.primary, rt(k.title)) \ #text(size: 0.94em, rt(k.body))]))
    #v(3mm)
  ]
  #if data.terms.len() > 0 [
    #eyebrow("Terms") #v(1mm)
    #set par(justify: false)
    #columns(2, gutter: 6mm)[
      #for (term, meaning) in data.terms [#text(font: ui, size: 8.6pt, weight: 700, fill: T.primary, rt(term)) — #text(size: 0.92em, rt(meaning)) \ #v(0.8mm)]]
  ]
  #if data.warnings.len() > 0 [
    #block(width: 100%, fill: T.warn-soft, radius: 2mm, inset: (x: 5mm, y: 3.5mm), above: 4mm)[
      #eyebrow("Watch out", color: T.warn) \
      #set par(justify: false)
      #for w in data.warnings [#text(font: ui, weight: 700, fill: T.warn, "✕ ") #rt(w) \ ]]
  ]
]

// ---------- Self-test ----------
#if data.checks.len() > 0 [
  #pagebreak()
  #heading(level: 1, outlined: true, bookmarked: true)[Self-test] #label("self-test")
  #block(width: 100%, fill: T.primary, radius: 2mm, inset: (x: 6mm, y: 4.5mm), below: 5mm)[
    #text(font: head, size: 22pt, weight: 700, fill: white, "Self-test") #h(3mm) #text(font: ui, size: 9pt, fill: white.transparentize(20%))[#data.checks.len() #if data.checks.len() == 1 [question] else [questions] · answers follow the questions]]
  #set par(justify: false)
  #grid(columns: (1fr, 1fr), column-gutter: 4mm, row-gutter: 4mm,
    ..data.checks.enumerate().map(((i, c)) => block(width: 100%, breakable: false, stroke: 0.7pt + T.rule, radius: 2mm, inset: 4mm)[
      #grid(columns: (8mm, 1fr), text(font: head, size: 12pt, weight: 700, fill: T.accent, str(i + 1)), [#text(weight: 600, rt(c.prompt)) #v(1mm) #text(font: ui, size: 7pt, fill: T.muted, c.section)])]))
  #v(5mm)
  #block(width: 100%, fill: T.accent-soft, radius: 2mm, inset: (x: 5mm, y: 4mm))[
    #eyebrow("Answers") #v(1.5mm)
    #columns(2, gutter: 6mm)[
      #for (i, c) in data.checks.enumerate() [#text(font: ui, weight: 700, fill: T.primary, str(i + 1)) #h(1mm) #rt(c.answer) \ #v(0.6mm)]]]
]

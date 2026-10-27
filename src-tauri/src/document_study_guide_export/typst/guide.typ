// oStudy study guide PDF template.
//
// The native exporter writes the effective guide to /data.json and compiles
// this file. Every learner- or model-authored string arrives as JSON data and
// is placed as text, never evaluated as markup. The only evaluated code is
// math the exporter translated from its bounded LaTeX subset (`m` runs).
#import "blocks.typ": *

#set document(title: data.title)
#set text(font: (T.body, "Libertinus Serif"), size: T.body-size, fill: T.ink, lang: "en", hyphenate: true)
#set par(justify: true, leading: 0.6em, spacing: 0.75em)
#show math.equation: set text(font: ("New Computer Modern Math",))

/// The room a section heading keeps for the reader's Practice button.
#let practice-width = 24mm
#let practice-height = 6.5mm

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
  // The overview, objectives and contents, set smaller when a long overview
  // would otherwise run off the cover onto a page of its own.
  #let cover-columns(scale) = {
    grid(columns: (1fr, 66mm), gutter: 12mm,
      [
        #set par(justify: false)
        #text(size: T.body-size * 1.2 * scale, rt(data.overview))
        #if data.objectives.len() > 0 [
          #v(5mm)
          #eyebrow("You will be able to") #v(1mm)
          #for (i, o) in data.objectives.enumerate() [
            #set text(size: T.body-size * scale)
            #grid(columns: (6mm, 1fr), text(font: ui, weight: 700, fill: T.accent, str(i + 1)), rt(o)) #v(0.6mm)]
        ]
      ],
      block(width: 100%, fill: T.accent-soft, radius: 2mm, inset: 5mm)[
        #eyebrow("Contents") #v(2mm)
        #set par(justify: false)
        #for entry in contents-entries.slice(0, calc.min(cover-limit, contents-entries.len())) [#contents-line(entry) #v(0.6mm)]
        #if contents-entries.len() > cover-limit [
          #text(font: ui, size: 8pt, fill: T.muted)[and #(contents-entries.len() - cover-limit) more on the next page]]
        #if data.cram.len() + data.checks.len() > 0 [
          #v(2mm)
          #if data.cram.len() > 0 [#contents-line((label: "cram", number: "", title: ((t: "Cram sheet"),))) #v(0.6mm)]
          #if data.checks.len() > 0 [#contents-line((label: "self-test", number: "", title: ((t: "Self-test"),)))]
        ]
      ])
  }
  #layout(size => {
    let room = size.height - 121mm
    let scale = 1
    for s in (1, 0.92, 0.85, 0.78, 0.72) {
      scale = s
      if measure(block(width: size.width, cover-columns(s))).height <= room { break }
    }
    cover-columns(scale)
  })
  #v(1fr)
  #hairline
  #v(2mm)
  #text(font: ui, size: 7.5pt, fill: T.muted)[From #data.source · #data.sectionCount #if data.sectionCount == 1 [section] else [sections] · Made with oStudy]
]

// ---------- Running pages ----------
#set page(paper: "a4", fill: paper, margin: (top: 18mm, bottom: 15mm, x: 16mm),
  header: context [
    #set text(font: ui, size: 7pt, fill: T.muted)
    #block(below: 1.6mm, [#text(weight: 600, fill: T.primary, data.title) #h(1fr) #data.source])
    #hairline
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
  block(above: 6mm, below: 3mm, sticky: true, grid(columns: (1fr, practice-width), column-gutter: 4mm, align: (left + bottom, right + bottom),
    [#anchor("section", s.key)#if name != none [#h #label(name)] else { h }],
    box(width: practice-width, height: practice-height)[#anchor("practice", s.key)]))
  // Figures go with the key points they illustrate: the text from there on
  // flows around them.
  let at = s.figureAfter
  let start = if s.blocks.len() == 0 { 0 } else if s.blocks.at(at).kind == "bullets" { at } else { at + 1 }
  for b in s.blocks.slice(0, start) { block-view(b) }
  flow(units-of(s.blocks.slice(start)), s.figures, number)
  if s.sources.len() > 0 { sources-view(s.key, s.sources) }
  if s.notes.len() > 0 {
    tile(fill: T.highlight-soft)[#eyebrow("My notes", color: T.highlight-ink) #v(1mm) #bullets(s.notes)]
  }
}

#show heading.where(level: 2): it => it.body
#show heading.where(level: 1): it => place(hide(it.body))

#if has-chapters {
  for (ci, chapter) in data.chapters.enumerate() {
    // Chapters run on from the one before, so a chapter's last page is not
    // left mostly empty; its band stays with its first section.
    block(width: 100%, fill: T.primary, radius: 2mm, inset: (x: 6mm, y: 4.5mm), above: if ci == 0 { 6mm } else { 9mm }, below: 4mm, breakable: false, sticky: true)[
      #anchor("chapter", chapter.key)#heading(level: 1, outlined: true, bookmarked: true)[#plain(chapter.title)] #label("chapter-" + str(ci + 1))
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
// The whole guide on one page: each section's key idea and key points under
// its number, then the terms and warnings, in three columns set as large as
// still fits the page. Key points come before terms: a long guide keeps fewer
// terms first, then is set smaller, and only then keeps fewer key points of
// each section (the first ones, which the writer puts first because they
// matter most).
#let cram-group(g, cap) = {
  block(above: 0pt, below: 1.6mm, sticky: true, {
    set par(justify: false, leading: 0.45em)
    text(font: ui, weight: 700, fill: T.accent, g.number)
    h(1.2mm)
    text(font: ui, weight: 700, fill: T.primary, rt(g.title))
  })
  for idea in g.ideas {
    block(width: 100%, fill: T.highlight-soft, radius: 1mm, inset: (x: 1.6mm, y: 1.1mm), above: 0pt, below: 1.2mm, {
      set par(justify: false, leading: 0.45em)
      text(weight: 600, rt(idea))
    })
  }
  let points = g.points.slice(0, calc.min(cap, g.points.len()))
  if points.len() > 0 {
    block(above: 0pt, below: 3mm, grid(columns: (2.6mm, 1fr), row-gutter: 0.9mm,
      ..points.map(p => (text(font: ui, weight: 700, fill: T.accent, "›"), {
        set par(justify: false, leading: 0.45em)
        rt(p)
      })).flatten()))
  }
}

#let cram-extras(term-cap) = {
  let terms = data.terms.slice(0, calc.min(term-cap, data.terms.len()))
  if terms.len() > 0 {
    block(above: 1mm, below: 1.6mm, sticky: true, eyebrow("Terms"))
    for (term, meaning) in terms {
      block(above: 0pt, below: 1mm, {
        set par(justify: false, leading: 0.45em)
        text(font: ui, weight: 700, fill: T.primary, rt(term))
        [ — ]
        rt(meaning)
      })
    }
    v(2mm)
  }
  if data.warnings.len() > 0 {
    block(width: 100%, fill: T.warn-soft, radius: 1mm, inset: (x: 1.8mm, y: 1.6mm), above: 1mm, below: 2mm, breakable: true, {
      set par(justify: false, leading: 0.45em)
      eyebrow("Watch out", color: T.warn)
      for w in data.warnings {
        block(above: 1.2mm, below: 0pt, grid(columns: (2.8mm, 1fr), text(font: ui, weight: 700, fill: T.warn, "✕"), rt(w)))
      }
    })
  }
}

#let cram-body(size, cap, term-cap) = {
  set text(size: size)
  if data.keyConcepts.len() > 0 {
    block(width: 100%, fill: T.primary, radius: 1.5mm, inset: (x: 2mm, y: 1.6mm), above: 0pt, below: 2.4mm, {
      set par(justify: false, leading: 0.45em)
      eyebrow("Key concepts", color: T.band-light)
      linebreak()
      text(fill: white, data.keyConcepts.map(c => rt(c)).join(text(fill: T.highlight, " ■ ")))
    })
  }
  for g in data.cram { cram-group(g, cap) }
  cram-extras(term-cap)
}

#if data.cram.len() + data.terms.len() + data.keyConcepts.len() > 0 [
  #pagebreak()
  #heading(level: 1, outlined: true, bookmarked: true)[Cram sheet] #label("cram") #anchor("cram", "cram")
  #text(font: head, size: 18pt, weight: 700, fill: T.primary, "Cram sheet") #h(3mm) #text(font: ui, size: 8.5pt, fill: T.muted, "The key ideas of the whole guide on one page.")
  #v(0.5mm) #line(length: 100%, stroke: 1.2pt + T.primary)
  #v(2mm)
  #layout(size => {
    // The page below the title; columns fill one after another, and a little
    // is kept back for the space a column loses where it breaks.
    let room = size.height - 16mm
    let gutter = 4mm
    let column = (size.width - 2 * gutter) / 3
    let fits(option) = measure(block(width: column, cram-body(..option))).height <= 3 * room - 12mm
    let options = ()
    for cap in (99, 4, 3, 2, 1) {
      for (size, term-cap) in ((8.6pt, 40), (8.6pt, 24), (7.8pt, 24), (7.8pt, 14), (7.1pt, 14), (7.1pt, 8), (6.6pt, 8), (6.6pt, 0), (6.2pt, 0)) {
        options.push((size, cap, term-cap))
      }
    }
    let choice = options.find(fits)
    if choice == none { choice = options.last() }
    // Laid out unbounded, in case the estimate is short: a full sheet then
    // runs on rather than printing over itself.
    columns(3, gutter: gutter, cram-body(..choice))
  })
]

// ---------- Self-test ----------
#if data.checks.len() > 0 [
  #pagebreak()
  #heading(level: 1, outlined: true, bookmarked: true)[Self-test] #label("self-test") #anchor("selfTest", "selfTest")
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

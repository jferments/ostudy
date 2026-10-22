// oStudy live study guide "At a glance" page.
//
// The reader's first page as a designed page in the PDF's theme: what the
// guide is about, what the learner will be able to do, its key concepts, its
// key ideas and what to watch out for. The reader keeps the guide's title and its links to each section as its
// own interactive parts, so the page holds none of them.
#import "blocks.typ": *

#set document(title: "At a glance")
#set page(width: 190mm, height: auto, margin: (x: 13mm, top: 12mm, bottom: 9mm), fill: paper)
#set text(font: (T.body, "Libertinus Serif"), size: T.body-size, fill: T.ink, lang: "en", hyphenate: true)
#set par(justify: false, leading: 0.64em, spacing: 0.85em)
#show math.equation: set text(font: ("New Computer Modern Math",))

#place(top + left, dx: -13mm, dy: -12mm, rect(width: 190mm, height: 2.4mm, fill: T.primary))
#place(top + left, dx: -13mm, dy: -9.6mm, rect(width: 46mm, height: 1.2mm, fill: T.highlight))

#eyebrow("At a glance")
#v(1.5mm)
#text(size: T.body-size * 1.2, rt(data.overview))

#if data.objectives.len() > 0 [
  #v(3mm)
  #eyebrow("You will be able to") #v(1mm)
  #for (i, o) in data.objectives.enumerate() [
    #grid(columns: (6mm, 1fr), text(font: ui, weight: 700, fill: T.accent, str(i + 1)), rt(o)) #v(0.6mm)]
]

#if data.keyConcepts.len() > 0 [
  #v(3mm)
  #block(width: 100%, fill: T.primary, radius: 2mm, inset: (x: 5mm, y: 4mm))[
    #eyebrow("Key concepts", color: T.band-light) #v(1.5mm)
    #grid(columns: (1fr, 1fr), column-gutter: 6mm, row-gutter: 1.6mm,
      ..data.keyConcepts.map(c => text(fill: white, [#text(fill: T.highlight, weight: 700, "■ ") #rt(c)])))]
]

#if data.keyIdeas.len() > 0 [
  #v(3mm)
  #eyebrow("Things to know") #v(1mm)
  #grid(columns: (1fr, 1fr), column-gutter: 3mm, row-gutter: 3mm,
    ..data.keyIdeas.enumerate().map(((i, k)) => block(width: 100%, fill: T.accent-soft, radius: 2mm, inset: 3.5mm, breakable: false)[
      #grid(columns: (6mm, 1fr),
        text(font: ui, size: 11pt, weight: 700, fill: T.accent, str(i + 1)),
        [#text(font: ui, size: 9pt, weight: 700, fill: T.primary, rt(k.title)) \ #text(size: 0.94em, rt(k.body))])]))
]

#if data.warnings.len() > 0 [
  #block(width: 100%, fill: T.warn-soft, radius: 2mm, inset: (x: 5mm, y: 3.5mm), above: 4mm)[
    #eyebrow("Watch out", color: T.warn) \
    #for w in data.warnings [#text(font: ui, weight: 700, fill: T.warn, "✕ ") #rt(w) \ ]]
]

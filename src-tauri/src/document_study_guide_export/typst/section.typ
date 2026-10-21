// oStudy live study guide section page.
//
// The reader shows one section at a time as this page, set in the same theme
// and blocks as the PDF. The reader keeps the section's heading, terms and
// check questions as its own interactive parts, so the page holds the body
// and figures only, on one page as tall as they need.
#import "blocks.typ": *

#set document(title: plain(data.section.title))
#set page(width: 190mm, height: auto, margin: (x: 13mm, top: 12mm, bottom: 9mm), fill: paper)
#set text(font: (T.body, "Libertinus Serif"), size: T.body-size, fill: T.ink, lang: "en", hyphenate: true)
#set par(justify: true, leading: 0.64em, spacing: 0.85em)
#show math.equation: set text(font: ("New Computer Modern Math",))

#place(top + left, dx: -13mm, dy: -12mm, rect(width: 190mm, height: 2.4mm, fill: T.primary))
#place(top + left, dx: -13mm, dy: -9.6mm, rect(width: 46mm, height: 1.2mm, fill: T.highlight))

#for b in data.section.blocks { block-view(b) }
#for (i, f) in data.section.figures.enumerate() { figure-view(f, data.number + "." + str(i + 1)) }

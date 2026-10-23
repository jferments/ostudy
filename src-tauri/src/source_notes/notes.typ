// oStudy source notes template.
//
// The native side writes notes the AI wrote from a learner's prompt to
// /data.json and compiles this file into the PDF a study set reads. Every
// string arrives as JSON data and is placed as text, never evaluated as
// markup. Colours and typefaces are the Navy study guide theme.
#import "themes.typ": themes

#let data = json("/data.json")
#let T = themes.navy

#set document(title: data.title)
#set page(paper: "us-letter", margin: (x: 22mm, y: 24mm), fill: T.paper,
  footer: context align(center, text(size: 8.5pt, fill: T.muted, counter(page).display())))
#set text(font: (T.body, "IBM Plex Sans"), size: 11pt, fill: T.ink, lang: "en")
#set par(justify: false, leading: 0.62em, spacing: 0.9em)

#block(below: 6mm, text(font: T.head, size: 22pt, weight: "bold", fill: T.primary, data.title))
#if data.prompt != "" {
  block(below: 8mm, text(size: 9pt, fill: T.muted, "Written from: " + data.prompt))
}

#for section in data.sections {
  block(above: 7mm, below: 3mm, sticky: true,
    text(font: T.head, size: 14pt, weight: "bold", fill: T.accent, section.heading))
  for paragraph in section.paragraphs {
    par(paragraph)
  }
  if section.keyPoints.len() > 0 {
    block(fill: T.accent-soft, inset: (x: 4mm, y: 3mm), radius: 2mm, width: 100%, above: 3mm, {
      text(font: T.ui, size: 9pt, weight: "bold", fill: T.accent, "Key points")
      list(..section.keyPoints.map(point => [#point]))
    })
  }
}

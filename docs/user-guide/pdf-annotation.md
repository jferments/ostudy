# Annotating PDF pages

The annotator lets you write, highlight, draw arrows and boxes, and type notes on the pages of a PDF in your library. The PDF itself is never changed: annotations are saved in your library as you work, page by page, and drawn over the page images made when the PDF was imported.

## Open a document

Open **Library → Documents** and choose **Annotate** from a PDF's **…** menu. **Annotate** appears once the import has drawn the document's pages and the document is not being processed; other document formats cannot be annotated yet.

The pages scroll in one column. **Back to documents** returns to the list, and so do Escape (when nothing is selected) and Android Back. The header shows **Saving…** and **Saved** as you work. If a save fails it shows **Not saved** with **Retry**, and leaving asks before discarding anything unsaved.

### From a card's source

A card made from a PDF names its source pages. In the card editor its **Source** panel has **Open page N**, and in Review the bar above the card shows **Source, page N** once the answer is showing (on a phone it is in **Card actions**). Either opens the PDF over the card at that page, outlined for a moment, with every annotation tool, so you can mark the passage while you read it again. Closing saves your marks and returns to the card; in Review the time spent reading is not counted as the card's answer time.

A source opens only while its PDF is in your library. Once the document is deleted, or for cards from a shared deck (`.studydeck` files never carry the document or its citations), the source stays plain text.

## Tools

| Tool | Key | What it does |
| --- | --- | --- |
| Select | V | Tap an annotation to select it, drag it to move it, and drag its handles to reshape it |
| Pen | P | Handwriting and drawing; a pen with pressure draws thicker when pressed harder |
| Highlighter | H | Drag across the PDF's text to mark whole lines of it; anywhere else, a wide, see-through stroke that keeps the text under it readable |
| Text | T | Tap where a note should go and type |
| Sticky note | S | Tap to place a small note that holds longer text and shows its first lines on the page |
| Line, Arrow, Box, Ellipse | L, A, R, O | Drag from one corner or end to the other |
| Eraser | E | Drag across annotations to remove each one it touches. A stylus's eraser end, if it has one, erases with any tool |

The row under the tools sets the color and size of what you draw next. With an annotation selected, the same row restyles it and adds **Make a card** and **Delete** (and **Edit text** for a text note, **Open note** for a sticky note). On a phone, **Undo** and **Redo** start that row, and **Color and size** opens the colors and sizes in a sheet.

Hold Shift while dragging a line or arrow to keep it at a multiple of 45 degrees, or while dragging a box or ellipse to make a square or circle.

**Undo** and **Redo** take back or repeat each stroke, move, restyle or erase while the annotator is open (Ctrl+Z, and Ctrl+Shift+Z or Ctrl+Y; Cmd on a Mac keyboard).

## Highlighting text

On a PDF with real text, a highlighter drag that starts on a word marks the text from there to wherever the drag ends, following the lines the way selecting text does, and the highlight keeps the words it covers so a screen reader can read them back. A tap on the text marks nothing. A drag that starts beside the text, a scanned page with no text in it, and the Android app draw an ordinary highlighter stroke instead.

## Text notes

Choose **Text**, tap or click the page, and type. Escape, Ctrl+Enter, or a tap elsewhere finishes the note; it is saved like any other annotation. To change a note, tap it with the Text tool, tap it again once it is selected with the Select tool, or press Enter while it is selected. A note emptied of text is removed. Drag a selected note's side handle to change how wide it wraps.

## Sticky notes

Choose **Sticky note**, then tap or click the page. A small square of colored paper appears there and a window opens for its text, which can run to many paragraphs. **Done**, Escape or Ctrl+Enter closes the window and saves the note; the square on the page shows the first lines. To read or change a note, tap it with the Sticky note tool, tap it again once it is selected with the Select tool, or press Enter while it is selected. **Delete note** removes it, and so does closing it with no text. Drag a sticky note with the Select tool to move it; the color and text-size choices restyle it.

## Cards from your marks

Select a highlight, a note or any other annotation and choose **Make a card**. A new-card editor opens beside the pages (over them on a phone) with the highlighted words, or the note's text, already in the answer; write the question and save. The card goes in the deck the document's cards were made in, or the deck of the last card you made from this document, and you can pick another deck in the editor. **Save + another** keeps the editor open for a second card from the same mark.

Each card made this way remembers its mark: its **Source** names the document and page. Once a document has cards made from its marks, **Study marked cards** in the annotator's header starts a practice session of them. A card stays when you erase its mark or delete the document, and it still counts as made from that document until the document is deleted.

## Zoom and scroll

The pages open at automatic zoom: fitted to the width of the window, but no larger than 125%, the size at which the page images look sharpest. Zoom with two fingers, Ctrl and the mouse wheel or a trackpad pinch, Ctrl+= and Ctrl+-, or the zoom buttons; Ctrl+0 or the percentage button returns to automatic zoom. Two fingers scroll, and so does a finger that starts beside the pages. The middle mouse button drags the view.

The page images are drawn at 120 dpi when a PDF is imported. When you zoom in past them, or on a high-density screen such as a tablet's, oStudy draws the part of the page you are looking at again from the PDF at the size it is shown, a moment after scrolling or zooming stops; until then the page shows its softer image. Nothing extra is saved. Annotations stay sharp at any zoom.

## Pens, fingers and phones

Every tool works by touch, on any screen size:

| Device | Drawing | Notes |
| --- | --- | --- |
| Android phone | A finger draws with every tool; strokes are shaped by speed rather than pressure | The shape tools and zoom commands fold into **Shapes** and **Zoom** menus. Typed notes use the on-screen keyboard and stay in view above it. Zoom in to write by hand comfortably |
| Android tablet with a stylus | The stylus draws with its pressure | Once a stylus touches a page, **Pen only** turns on: the stylus draws, fingers scroll and zoom, and a palm resting on the screen is ignored while the stylus is down. Turn **Pen only** off to draw with a finger again |
| Desktop with a mouse or trackpad | The mouse draws; strokes are shaped by speed | All keyboard shortcuts are available |
| Linux with a graphics tablet | The pen draws like a mouse | The Linux webview reports a tablet pen as a mouse, without pressure, so strokes are shaped by speed |

Keyboard shortcuts need a hardware keyboard. Handwriting is stored as ink, not as text: handwriting recognition is not available yet.

## Keyboard and screen readers

With the Select tool, Tab moves through the annotations on the pages in view. Focusing one selects it, the arrow keys move it by one point (ten with Shift), Delete removes it, and Enter opens a text or sticky note. The tools are toolbar buttons that announce their keyboard shortcuts.

## Where annotations are kept

Annotations belong to their document in your library. A whole-library backup includes them, and restoring the backup restores them. Device sync does not send them yet, so each device keeps its own. A build of oStudy from before text highlighting (or, for sticky notes, from before sticky notes) opens a library that has them, but leaves them out until a newer build opens it again. Deleting a document deletes its annotations. Which marks your cards were made from stays on the device too: device sync and shared decks do not carry it.

Storing annotations needs library schema 8, and cards from marks need schema 10. The first launch of a build with the annotator upgrades the library and keeps a pre-upgrade checkpoint; see [Library upgrades](library-upgrades.md#the-document-annotations-update-schema-8).

## Not available yet

- Having AI draft the question for a card from a mark.
- Opening the marked passage from a card's Source.

- Handwriting recognition, and turning handwriting into searchable text.
- Highlighting that snaps to text on Android.
- Saving or exporting a copy of the PDF with its annotations.
- Erasing part of a stroke, and selecting several annotations at once.
- Annotating documents other than PDFs, such as Word or PowerPoint files and image sets.

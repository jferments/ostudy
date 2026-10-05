# Annotating PDF pages

The annotator lets you write, highlight, draw arrows and boxes, and type notes on the pages of a PDF in your library. The PDF itself is never changed: annotations are saved in your library as you work, page by page, and drawn over the page images made when the PDF was imported.

## Open a document

Open **Library → Documents** and choose **Annotate** (the pen icon) beside a PDF. A PDF inside a collection has an **Annotate** button in its collection's source list. The button appears once the import has drawn the document's pages; other document formats cannot be annotated yet.

The pages scroll in one column. **Back to documents** returns to the list, and so do Escape (when nothing is selected) and Android Back. The header shows **Saving…** and **Saved** as you work. If a save fails it shows **Not saved** with **Retry**, and leaving asks before discarding anything unsaved.

## Tools

| Tool | Key | What it does |
| --- | --- | --- |
| Select | V | Tap an annotation to select it, drag it to move it, and drag its handles to reshape it |
| Pen | P | Handwriting and drawing; a pen with pressure draws thicker when pressed harder |
| Highlighter | H | A wide, see-through stroke that keeps the text under it readable |
| Text | T | Tap where a note should go and type |
| Line, Arrow, Box, Ellipse | L, A, R, O | Drag from one corner or end to the other |
| Eraser | E | Drag across annotations to remove each one it touches. A stylus's eraser end, if it has one, erases with any tool |

The row under the tools sets the color and size of what you draw next. With an annotation selected, the same row restyles it and adds **Delete** (and **Edit text** for a note). On a phone, **Undo** and **Redo** start that row, and **Color and size** opens the colors and sizes in a sheet.

Hold Shift while dragging a line or arrow to keep it at a multiple of 45 degrees, or while dragging a box or ellipse to make a square or circle.

**Undo** and **Redo** take back or repeat each stroke, move, restyle or erase while the annotator is open (Ctrl+Z, and Ctrl+Shift+Z or Ctrl+Y; Cmd on a Mac keyboard).

## Text notes

Choose **Text**, tap or click the page, and type. Escape, Ctrl+Enter, or a tap elsewhere finishes the note; it is saved like any other annotation. To change a note, tap it with the Text tool, tap it again once it is selected with the Select tool, or press Enter while it is selected. A note emptied of text is removed. Drag a selected note's side handle to change how wide it wraps.

## Zoom and scroll

The pages open at automatic zoom: fitted to the width of the window, but no larger than 125%, the size at which the page images look sharpest. Zoom with two fingers, Ctrl and the mouse wheel or a trackpad pinch, Ctrl+= and Ctrl+-, or the zoom buttons; Ctrl+0 or the percentage button returns to automatic zoom. Two fingers scroll, and so does a finger that starts beside the pages. The middle mouse button drags the view.

The page images are drawn at 120 dpi when a PDF is imported, so pages look softer when zoomed in past 125%. Annotations stay sharp at any zoom.

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

With the Select tool, Tab moves through the annotations on the pages in view. Focusing one selects it, the arrow keys move it by one point (ten with Shift), Delete removes it, and Enter edits a text note. The tools are toolbar buttons that announce their keyboard shortcuts.

## Where annotations are kept

Annotations belong to their document in your library. A whole-library backup includes them, and restoring the backup restores them. Device sync does not send them yet, so each device keeps its own. Deleting a document deletes its annotations.

Storing annotations needs library schema 7. The first launch of a build with the annotator upgrades the library and keeps a pre-upgrade checkpoint; see [Library upgrades](library-upgrades.md#the-document-annotations-update-schema-7).

## Not available yet

- Handwriting recognition, and turning handwriting into searchable text.
- Highlighting that snaps to the PDF's own text lines.
- Saving or exporting a copy of the PDF with its annotations.
- Erasing part of a stroke, and selecting several annotations at once.
- Annotating documents other than PDFs, such as Word or PowerPoint files and image collections.

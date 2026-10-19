# Layout fixtures

`textbook-two-column.pdf` (a two-column textbook page with a running header,
page number, hyphenation and a footnote) and `table-handout.pdf` (a ruled
four-column table between two paragraphs) were generated for this repository
with ReportLab 5.0 from original text. They embed subsets of the DejaVu Serif
and DejaVu Sans fonts (Bitstream Vera and public-domain changes; free to
embed and redistribute). They exercise PDF layout reading: a two-column page
must not be read as a table, and a real table must stay a table.

`ruled-tables-wrapped.pdf` (two five-column tables drawn cell by cell, with
wrapped cells, a caption between them and a frame around the page) was
generated with ReportLab 5.0 from original text in the standard Helvetica
font, which is not embedded. Each row's values must stay together and the
caption must survive between the tables.

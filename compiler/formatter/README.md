# Candy Formatter

This crate contains an opinionated formatter for Candy.
The formatter cannot be configured.

The maximum line width is 100 columns.
This width is measured based on [Unicode Standard Annex #11](http://www.unicode.org/reports/tr11/) using [<kbd>unicode-width</kbd>](https://crates.io/crates/unicode-width).

Notable features:

- trailing commas are inserted/removed as needed
- comments might be moved to the other side of a dot/colon/arrow/etc.
- parentheses are inserted/removed as needed
- empty lines in bodies are kept up to a limit of two subsequent empty lines

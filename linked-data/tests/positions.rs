// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Feature tests for turning byte offsets into lines and columns.
//!
//! Parsers report where a problem is as a byte offset in the document, because that is what
//! a parser reading bytes knows. A person fixing the document needs a line and a column, as
//! a text editor shows them. The host holds the document, so the host does the conversion,
//! and every parser is spared it.

use linked_data::positions::{LineIndex, Position};

/// A Turtle document of three lines. The second line contains `ä` twice and `ö` once, each
/// two bytes long in UTF-8.
const DOCUMENT: &str =
	"@prefix ex: <https://example.org/> .\nex:säätö ex:p \"o\" .\nex:s ex:p ex:o .\n";

/// The byte offset of the first occurrence of `text` in the document. Every call in this
/// file looks for text the document contains.
fn offset_of(text: &str) -> u64 {
	DOCUMENT
		.find(text)
		.and_then(|offset| u64::try_from(offset).ok())
		.unwrap_or(u64::MAX)
}

/// Lines and columns count from one, as text editors count them.
#[test]
fn places_the_first_byte_at_line_one_column_one() {
	let index = LineIndex::new(DOCUMENT.as_bytes());

	assert_eq!(index.position(0), Some(Position { line: 1, column: 1 }));
}

/// The byte after a line feed starts the next line.
#[test]
fn starts_a_new_line_after_a_line_feed() {
	let index = LineIndex::new(DOCUMENT.as_bytes());

	assert_eq!(
		index.position(offset_of("ex:säätö")),
		Some(Position { line: 2, column: 1 })
	);
}

/// Columns count characters, not bytes. On the second line, `ex:p` follows `ex:säätö `,
/// which is nine characters but twelve bytes, so `ex:p` starts in column ten.
#[test]
fn counts_columns_in_characters_rather_than_bytes() {
	let index = LineIndex::new(DOCUMENT.as_bytes());

	assert_eq!(
		index.position(offset_of("ex:p")),
		Some(Position {
			line: 2,
			column: 10
		})
	);
}

/// A carriage return before a line feed belongs to the line it ends, so documents written
/// with Windows line endings get the same line numbers as any other.
#[test]
fn treats_a_carriage_return_and_line_feed_as_one_line_ending() {
	let index = LineIndex::new(b"<a> <b> <c> .\r\n<d> <e> <f> .\r\n");

	assert_eq!(index.position(15), Some(Position { line: 2, column: 1 }));
}

/// An offset past the end of the document has no position, and the host reports the byte
/// offset alone instead of inventing one.
#[test]
fn gives_no_position_past_the_end_of_the_document() {
	let index = LineIndex::new(DOCUMENT.as_bytes());

	assert_eq!(index.position(10_000), None);
}

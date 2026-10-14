// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Turning byte offsets in a document into lines and columns.
//!
//! Parsers report where something is as a byte offset, which is what a parser reading
//! bytes knows. People fixing a document work in lines and columns. A [`LineIndex`] finds
//! where each line starts once, so that converting any number of offsets afterwards is a
//! search, not a scan of the whole document.

/// A place in a document, as a text editor shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Position {
	/// The line, counting from one.
	pub line: u64,
	/// The column, counting characters from one. A character is a Unicode scalar value,
	/// however many bytes it takes in the document.
	pub column: u64,
}

/// The byte offset at which each line of a document starts.
///
/// A line ends with a line feed. A carriage return before the line feed belongs to the line
/// it ends, so documents with Windows line endings are numbered the same as any other.
/// Columns are counted by decoding the document as UTF-8, which every syntax in the Turtle
/// family requires. For a document in another encoding the column may be off, but the line
/// is still right.
#[derive(Debug)]
pub struct LineIndex<'document> {
	/// The document, as bytes.
	document: &'document [u8],
	/// The byte offset at which each line starts. The first line starts at zero.
	line_starts: Vec<usize>,
}

impl<'document> LineIndex<'document> {
	/// Finds where each line of `document` starts.
	#[must_use]
	pub fn new(document: &'document [u8]) -> Self {
		let line_starts = std::iter::once(0)
			.chain(
				document
					.iter()
					.enumerate()
					.filter(|&(_, &byte)| byte == b'\n')
					.map(|(offset, _)| offset + 1),
			)
			.collect();
		Self {
			document,
			line_starts,
		}
	}

	/// The line and column of the byte at `offset`.
	///
	/// The offset just past the last byte has a position, the end of the document. Any
	/// offset beyond that has none, and the host should report the byte offset alone.
	#[must_use]
	pub fn position(&self, offset: u64) -> Option<Position> {
		let offset = usize::try_from(offset).ok()?;
		if offset > self.document.len() {
			return None;
		}
		let line_index = self.line_starts.partition_point(|&start| start <= offset) - 1;
		let line_start = self.line_starts[line_index];
		let characters = String::from_utf8_lossy(&self.document[line_start..offset])
			.chars()
			.count();
		Some(Position {
			line: u64::try_from(line_index).ok()? + 1,
			column: u64::try_from(characters).ok()? + 1,
		})
	}
}

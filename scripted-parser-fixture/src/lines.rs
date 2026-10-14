// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Finding the lines of a document, as spans of bytes.
//!
//! The scenarios place each statement on a line of the document the host sent. The host
//! turns spans back into lines and columns with its own line index, so splitting on line
//! feeds here, as the host does, lets a test predict exactly which line it will report.

use naamio_space_kit_rdf::prelude::Span;

/// The span of every line in `document`, in order. A span covers its line without the line
/// feed that ends it.
pub fn spans(document: &[u8]) -> Vec<Span> {
	let mut spans = Vec::new();
	let mut start: u64 = 0;
	for line in document.split(|&byte| byte == b'\n') {
		let length = u64::try_from(line.len()).unwrap_or(u64::MAX);
		spans.push(Span {
			start,
			end: start.saturating_add(length),
		});
		start = start.saturating_add(length).saturating_add(1);
	}
	spans
}

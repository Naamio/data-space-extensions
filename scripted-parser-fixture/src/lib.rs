// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! A parser component for the extension host's tests, whose statements are scripted rather
//! than parsed.
//!
//! The component exports the `parser` interface of the `linked-data:rdf` contract, but
//! parses no syntax. It plays the scenario named by the media type the host passes, such as
//! `scenario/people`, reporting that scenario's statements and ending with its outcome. Each
//! statement carries the span of one line of the document, so the host's handling of
//! positions can be checked against known lines.
//!
//! The fixture is written with the Rust kit, as any syntax extension would be. The kit
//! reads the document from its stream and writes the statements and the outcome, so the
//! host's tests exercise the same export code that every extension built with the kit
//! runs.
//!
//! [`plays`] holds the scenarios, and [`lines`] finds the lines of a document.

mod lines;
mod plays;

use naamio_space_kit_rdf::prelude::*;

/// The fixture's syntax: a scenario to play, named by the media type.
struct ScriptedParser;

impl Syntax for ScriptedParser {
	/// Describes the fixture as a syntax of its own, with a media type no real document
	/// uses, and claims every feature so that a host never refuses it for lacking one.
	fn description() -> SyntaxDescription {
		SyntaxDescription {
			name: "Scripted parser fixture".to_owned(),
			version: "0.0".to_owned(),
			media_types: vec!["application/x-scripted-parser-fixture".to_owned()],
			file_extensions: vec!["scenario".to_owned()],
			features: SyntaxFeatures::all(),
		}
	}

	/// Plays the scenario the options name, with spans taken from the lines of `document`.
	fn read(document: &[u8], options: &ParseOptions) -> Reading {
		plays::play(options.media_type.as_deref(), &lines::spans(document))
	}
}

export_syntax!(ScriptedParser);

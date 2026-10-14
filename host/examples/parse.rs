// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Parses a document through a parser component, and prints the quads or the failure.
//!
//! ```shell
//! cargo run --example parse -- [--component <parser.wasm>] [--base <iri>] [--media-type <type>] <document>
//! ```
//!
//! The component reads the document's syntax and nothing more. Everything this example
//! prints beyond that is the host's work: the quads, with every Internationalized Resource
//! Identifier (IRI) resolved and every prefix expanded, and, when something is wrong, the
//! line and column of the problem, worked out by the host from the byte span the component
//! or the interpreter reported.
//!
//! Without `--component`, the example uses the Turtle component built from the
//! `components/turtle-parser` submodule. The samples beside this file show a successful
//! parse (`profile.ttl`), a syntax error found by the component (`broken.ttl`), and a
//! statement refused by the host (`undeclared-prefix.ttl`).

use std::path::PathBuf;
use std::process::ExitCode;

use host::parsers::{ParseFailure, ParseOptions, ParseReport};
use host::runtime::ExtensionHost;
use linked_data::dataset::Dataset;
use linked_data::oxrdf::GraphNameRef;
use linked_data::positions::Position;

/// The path, relative to this crate, of the Turtle component that building the submodule
/// writes.
const TURTLE_COMPONENT: &str =
	"../components/turtle-parser/target/wasm32-wasip3/release/turtle_parser.wasm";

/// The example's command-line arguments: the component, the base, the media type and the
/// document to parse.
#[derive(Debug)]
struct Arguments {
	/// The parser component to load.
	component: PathBuf,
	/// The base IRI to parse the document against, usually the address it came from.
	base: Option<String>,
	/// The media type to tell the parser the document has.
	media_type: String,
	/// The document to parse.
	document: PathBuf,
}

#[tokio::main]
async fn main() -> ExitCode {
	match arguments() {
		Ok(arguments) => run(&arguments).await,
		Err(problem) => {
			eprintln!("{problem}");
			eprintln!(
				"usage: cargo run --example parse -- [--component <parser.wasm>] [--base <iri>] \
				 [--media-type <type>] <document>"
			);
			ExitCode::FAILURE
		}
	}
}

/// Reads the command-line arguments.
fn arguments() -> Result<Arguments, String> {
	let mut component = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(TURTLE_COMPONENT);
	let mut base = None;
	let mut media_type = "text/turtle".to_owned();
	let mut document = None;

	let mut given = std::env::args().skip(1);
	while let Some(argument) = given.next() {
		let mut value = |name: &str| given.next().ok_or_else(|| format!("{name} needs a value"));
		match argument.as_str() {
			"--component" => component = PathBuf::from(value("--component")?),
			"--base" => base = Some(value("--base")?),
			"--media-type" => media_type = value("--media-type")?,
			_ if document.is_none() => document = Some(PathBuf::from(argument)),
			_ => return Err(format!("unexpected argument `{argument}`")),
		}
	}

	let document = document.ok_or("no document was given")?;
	Ok(Arguments {
		component,
		base,
		media_type,
		document,
	})
}

/// Loads the component, parses the document, and prints what happened.
async fn run(arguments: &Arguments) -> ExitCode {
	let loaded = std::fs::read(&arguments.component).map_err(|error| {
		format!(
			"the component at {} could not be read ({error}). Build the Turtle component with \
			 `cargo build --release --target wasm32-wasip3` in components/turtle-parser.",
			arguments.component.display()
		)
	});
	let document = std::fs::read(&arguments.document).map_err(|error| {
		format!(
			"{} could not be read: {error}",
			arguments.document.display()
		)
	});
	let (component, document) = match (loaded, document) {
		(Ok(component), Ok(document)) => (component, document),
		(Err(problem), _) | (_, Err(problem)) => {
			eprintln!("{problem}");
			return ExitCode::FAILURE;
		}
	};

	let parser = match ExtensionHost::new().and_then(|host| host.load_parser(&component)) {
		Ok(parser) => parser,
		Err(error) => {
			eprintln!("{error}");
			return ExitCode::FAILURE;
		}
	};
	let options = ParseOptions {
		base: arguments.base.clone(),
		media_type: Some(arguments.media_type.clone()),
	};
	let mut dataset = Dataset::new();
	let outcome = parser.parse(&document, &options, &mut dataset).await;

	print_quads(&dataset);
	match outcome {
		Ok(report) => {
			print_report(&report);
			ExitCode::SUCCESS
		}
		Err(failure) => {
			print_failure(&failure, &String::from_utf8_lossy(&document));
			ExitCode::FAILURE
		}
	}
}

/// Prints every quad in the dataset as N-Quads, on standard output. After a failure, these
/// are the quads taken before it.
fn print_quads(dataset: &Dataset) {
	for quad in dataset.quads() {
		match quad.graph_name {
			GraphNameRef::DefaultGraph => {
				println!("{} {} {} .", quad.subject, quad.predicate, quad.object);
			}
			graph_name => println!(
				"{} {} {} {graph_name} .",
				quad.subject, quad.predicate, quad.object
			),
		}
	}
}

/// Prints the summary of a successful parse, and each warning with where it is, on standard
/// error, so the quads on standard output stay valid N-Quads.
fn print_report(report: &ParseReport) {
	eprintln!(
		"\n{} quads read, {} added to the dataset.",
		report.quads_read, report.quads_added
	);
	for warning in &report.warnings {
		eprintln!("warning{}: {}", at(warning.position), warning.message);
	}
}

/// Prints why the parse failed, on standard error, with the offending line of the document
/// and a caret under the column whenever the failure points into the document.
fn print_failure(failure: &ParseFailure, document: &str) {
	let (source, position) = match failure {
		ParseFailure::Syntax { position, .. } => ("the parser component", *position),
		ParseFailure::Interpretation { position, .. }
		| ParseFailure::LimitReached { position, .. } => ("the host", *position),
		_ => ("the host", None),
	};
	eprintln!("\nThe parse stopped. Reported by {source}: {failure}");
	if let Some(position) = position {
		let line = document
			.lines()
			.nth(usize::try_from(position.line.saturating_sub(1)).unwrap_or(usize::MAX))
			.unwrap_or_default();
		eprintln!("\n{:>5} | {line}", position.line);
		eprintln!("      | {}", caret_under(line, position.column));
	}
}

/// Describes a position for a message, as " at 3:9", or nothing when it is unknown.
fn at(position: Option<Position>) -> String {
	position.map_or_else(String::new, |position| {
		format!(" at {}:{}", position.line, position.column)
	})
}

/// The line drawn under a line of a document to point at `column`, counting characters from
/// one.
///
/// Each character before the column becomes a space, except a tab, which stays a tab, so the
/// caret lines up under the character however the terminal draws tabs and however many
/// bytes each character takes.
fn caret_under(line: &str, column: u64) -> String {
	let before = usize::try_from(column.saturating_sub(1)).unwrap_or(usize::MAX);
	let mut caret: String = line
		.chars()
		.take(before)
		.map(|character| if character == '\t' { '\t' } else { ' ' })
		.collect();
	caret.push('^');
	caret
}

#[cfg(test)]
mod tests {
	use super::caret_under;

	/// The caret sits under the first character when the problem is in column one.
	#[test]
	fn points_at_the_first_column() {
		assert_eq!(caret_under("<s> <p> \"open .", 1), "^");
	}

	/// Each character before the column takes one space, whatever its width in bytes, so the
	/// caret lines up under multi-byte text such as `ä`.
	#[test]
	fn counts_characters_rather_than_bytes() {
		assert_eq!(caret_under("ex:säätö oops", 10), "         ^");
	}

	/// A tab before the column is copied as a tab, so the caret lines up however wide the
	/// terminal draws tabs.
	#[test]
	fn keeps_tabs_so_the_caret_lines_up() {
		assert_eq!(caret_under("\tfoaf:name oops", 12), "\t          ^");
	}
}

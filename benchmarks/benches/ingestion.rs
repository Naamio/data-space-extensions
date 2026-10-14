// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Times turning a document into a dataset by each path, at three sizes.
//!
//! The `turtle` group tests the claim the `linked-data:rdf` contract rests on: that
//! streaming typed statements across the component boundary is faster than passing text for
//! the host to parse again. It times three paths over the same generated Turtle:
//!
//! - `typed`: the benchmark's component streams typed statements, and the extension host
//!   interprets them into the dataset.
//! - `text`: the same component, running the same grammar, writes those statements as
//!   Turtle, and the host parses that text into the dataset.
//! - `native`: the host parses the document itself with oxttl, with no component.
//!
//! The `n-triples` group times the Rust kit's and the JavaScript kit's N-Triples examples
//! against native N-Triples parsing, which shows what writing an extension in TypeScript
//! costs compared with Rust.
//!
//! Every timing includes creating a fresh instance of the component, as the host does for
//! each document, and creating the dataset, so each path pays for the same setup. Throughput
//! is reported in triples per second. Run with `cargo bench -p benchmarks`; the JavaScript
//! example needs Node and pnpm, as the host's tests do.

// The host's test support builds components and loads them into a host. The benchmark uses
// the same code, so the components it times are built the way the host's tests build them.
#[path = "../../host/tests/support/mod.rs"]
mod support;

use std::hint::black_box;
use std::time::Duration;

use benchmarks::documents::{self, BASE, TRIPLES_PER_PERSON};
use benchmarks::native;
use benchmarks::text::TextParserComponent;
use criterion::{BenchmarkId, Criterion, Throughput};
use host::parsers::{ParseFailure, ParseOptions, ParseReport, ParserComponent};
use linked_data::dataset::Dataset;
use support::TestError;
use tokio::runtime::Runtime;

/// The number of people the documents describe at each size, giving documents of one
/// thousand, ten thousand and one hundred thousand triples. Three sizes show whether a path's
/// cost is dominated by setup, which matters most for small documents, or by the work per
/// triple.
const SIZES: [usize; 3] = [100, 1_000, 10_000];

/// The number of timed samples criterion takes at each size. It is lower than criterion's
/// default of a hundred, because the largest documents take long enough per run that a
/// hundred samples would make the benchmark take many minutes, and ten already give a
/// stable median.
const SAMPLES: usize = 10;

/// The benchmark's component, which exports both the typed and the text interface.
const COMPONENT: (&str, &str) = ("benchmarks/turtle-text-parser", "turtle_text_parser.wasm");

/// Builds and loads every component, checks that each path produces the expected dataset,
/// then runs the timings.
///
/// Criterion's usual `criterion_main!` cannot return an error, and building a component can
/// fail, so `main` is written out to report a failed build with its cause.
#[expect(
	clippy::significant_drop_tightening,
	reason = "the runner is used until its final summary; only the return value follows it"
)]
fn main() -> Result<(), TestError> {
	let runtime = Runtime::new()?;
	let mut criterion = Criterion::default()
		.sample_size(SAMPLES)
		.measurement_time(Duration::from_secs(10))
		.configure_from_args();

	time_turtle_paths(&mut criterion, &runtime)?;
	time_n_triples_kits(&mut criterion, &runtime)?;

	criterion.final_summary();
	Ok(())
}

/// The options the typed path parses with: the base the generated documents are written
/// against, and Turtle's media type.
fn turtle_options() -> ParseOptions {
	ParseOptions {
		base: Some(BASE.to_owned()),
		media_type: Some("text/turtle".to_owned()),
	}
}

/// Parses `document` with `parser` into a new dataset: one run of a component path, as
/// each timed iteration makes it.
fn parse_fresh(
	runtime: &Runtime,
	parser: &ParserComponent,
	document: &[u8],
	options: &ParseOptions,
) -> Result<ParseReport, ParseFailure> {
	runtime.block_on(parser.parse(document, options, &mut Dataset::new()))
}

/// Fails unless `added`, the quads a path reports adding, is the number of triples the
/// generated document states. A path that silently stored nothing would otherwise time as
/// the fastest.
fn check_size(path: &str, added: usize, people: usize) -> Result<(), TestError> {
	let expected = people * TRIPLES_PER_PERSON;
	if added == expected {
		return Ok(());
	}
	Err(format!("the {path} path added {added} quads where the document states {expected}").into())
}

/// Times the typed, text and native paths over generated Turtle.
#[expect(
	clippy::significant_drop_tightening,
	reason = "the group is used until it is finished; only the return value follows it"
)]
fn time_turtle_paths(criterion: &mut Criterion, runtime: &Runtime) -> Result<(), TestError> {
	let (package, artefact) = COMPONENT;
	let typed = support::load_parser(package, artefact)?;
	let text = TextParserComponent::load(&std::fs::read(support::build_rust_component(
		package, artefact,
	)?)?)?;
	let options = turtle_options();

	let mut group = criterion.benchmark_group("turtle");
	for people in SIZES {
		let document = documents::turtle(people);
		let bytes = document.as_bytes();
		let triples = people * TRIPLES_PER_PERSON;

		let report = parse_fresh(runtime, &typed, bytes, &options)?;
		check_size("typed", usize::try_from(report.quads_added)?, people)?;
		check_size(
			"text",
			runtime.block_on(text.parse(bytes, BASE, &mut Dataset::new()))?,
			people,
		)?;
		check_size(
			"native",
			native::read_turtle(bytes, BASE, &mut Dataset::new())?,
			people,
		)?;

		group.throughput(Throughput::Elements(triples_as_elements(triples)));
		group.bench_function(BenchmarkId::new("typed", triples), |bencher| {
			bencher.iter(|| black_box(parse_fresh(runtime, &typed, bytes, &options)));
		});
		group.bench_function(BenchmarkId::new("text", triples), |bencher| {
			bencher
				.iter(|| black_box(runtime.block_on(text.parse(bytes, BASE, &mut Dataset::new()))));
		});
		group.bench_function(BenchmarkId::new("native", triples), |bencher| {
			bencher.iter(|| black_box(native::read_turtle(bytes, BASE, &mut Dataset::new())));
		});
	}
	group.finish();
	Ok(())
}

/// Times the two kits' N-Triples examples, and native N-Triples parsing, over generated
/// N-Triples.
#[expect(
	clippy::significant_drop_tightening,
	reason = "the group is used until it is finished; only the return value follows it"
)]
fn time_n_triples_kits(criterion: &mut Criterion, runtime: &Runtime) -> Result<(), TestError> {
	let examples: [(&str, ParserComponent); 2] = [
		(
			"rust-kit",
			support::load_parser("kits/rust/examples/n-triples", "n_triples_extension.wasm")?,
		),
		(
			"javascript-kit",
			support::load_javascript_parser(
				"kits/javascript",
				"examples/n-triples",
				"n_triples_extension.wasm",
			)?,
		),
	];
	let options = ParseOptions {
		base: None,
		media_type: Some("application/n-triples".to_owned()),
	};

	let mut group = criterion.benchmark_group("n-triples");
	for people in SIZES {
		let document = documents::n_triples(people);
		let bytes = document.as_bytes();
		let triples = people * TRIPLES_PER_PERSON;

		for (kit, example) in &examples {
			let report = parse_fresh(runtime, example, bytes, &options)?;
			check_size(kit, usize::try_from(report.quads_added)?, people)?;
		}
		check_size(
			"native",
			native::read_n_triples(bytes, &mut Dataset::new())?,
			people,
		)?;

		group.throughput(Throughput::Elements(triples_as_elements(triples)));
		for (kit, example) in &examples {
			group.bench_function(BenchmarkId::new(*kit, triples), |bencher| {
				bencher.iter(|| black_box(parse_fresh(runtime, example, bytes, &options)));
			});
		}
		group.bench_function(BenchmarkId::new("native", triples), |bencher| {
			bencher.iter(|| black_box(native::read_n_triples(bytes, &mut Dataset::new())));
		});
	}
	group.finish();
	Ok(())
}

/// `triples` as criterion's element count. A count that does not fit, which no generated
/// document reaches, is reported as the largest count criterion accepts.
fn triples_as_elements(triples: usize) -> u64 {
	u64::try_from(triples).unwrap_or(u64::MAX)
}

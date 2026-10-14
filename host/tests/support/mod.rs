// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Building the components the host's feature tests run.
//!
//! Each component is a package in this repository or in a submodule, built for WebAssembly
//! with this workspace's pinned toolchain. A test asks for a component by the package's
//! directory, and the first request in a test run builds it; Cargo rebuilds it only when its
//! source has changed.
//!
//! Every component written in Rust is built into a directory under this workspace's
//! `target/components`, named after its package. A package may be standalone or a member of
//! another workspace, such as the kit's examples, and building into a known directory finds
//! the component in the same place either way.
//!
//! A component written in TypeScript is built by its pnpm workspace's own `build` script,
//! which writes it to the package's `dist` directory. Building one needs Node and pnpm.

use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock, PoisonError};

use host::parsers::ParserComponent;
use host::runtime::ExtensionHost;

/// A boxed error type broad enough for anything a test might fail on.
pub type TestError = Box<dyn Error + Send + Sync>;

/// Builds the component in `package`, a directory relative to this repository's root, and
/// loads it into a new extension host. `artefact` is the file Cargo writes, named after the
/// package with hyphens replaced by underscores.
pub fn load_parser(package: &str, artefact: &str) -> Result<ParserComponent, TestError> {
	load(&build_rust_component(package, artefact)?)
}

/// Builds the component in `package`, as [`load_parser`] does, and returns the path of the
/// file Cargo writes, for a test that loads the component some other way.
pub fn build_rust_component(package: &str, artefact: &str) -> Result<PathBuf, TestError> {
	build(package, || run_cargo_build(package, artefact))
}

/// Builds the TypeScript component in `package`, a directory relative to the pnpm
/// `workspace`, which is itself relative to this repository's root, and loads it into a new
/// extension host. `artefact` is the file the package's `build` script writes to `dist`.
#[allow(
	dead_code,
	reason = "each test file compiles its own copy of this module, and only some load TypeScript \
		components; `expect` would fail in the files that do"
)]
pub fn load_javascript_parser(
	workspace: &str,
	package: &str,
	artefact: &str,
) -> Result<ParserComponent, TestError> {
	let directory = format!("{workspace}/{package}");
	load(&build(&directory, || {
		run_pnpm_build(workspace, package, artefact)
	})?)
}

/// Loads the component at `path` into a new extension host.
fn load(path: &Path) -> Result<ParserComponent, TestError> {
	let host = ExtensionHost::new()?;
	let component = std::fs::read(path)?;

	Ok(host.load_parser(&component)?)
}

/// The outcome of building each package so far in this test run, by package directory.
type Builds = HashMap<String, Result<PathBuf, String>>;

/// The path to the built component, building it with `run` on the first request for
/// `package`.
///
/// Each package is built once per test run. A failed build is remembered too, so every test
/// that needs the package reports the same failure instead of building it again.
fn build(
	package: &str,
	run: impl FnOnce() -> Result<PathBuf, String>,
) -> Result<PathBuf, TestError> {
	static BUILDS: OnceLock<Mutex<Builds>> = OnceLock::new();

	let outcome = BUILDS
		.get_or_init(|| Mutex::new(HashMap::new()))
		.lock()
		.unwrap_or_else(PoisonError::into_inner)
		.entry(package.to_owned())
		.or_insert_with(run)
		.clone();

	outcome.map_err(Into::into)
}

/// Installs the pnpm `workspace`'s locked dependencies, runs the `build` script of
/// `package` within it, and returns the path of the component the script writes.
fn run_pnpm_build(workspace: &str, package: &str, artefact: &str) -> Result<PathBuf, String> {
	let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("..")
		.join(workspace);
	let pnpm = std::env::var("PNPM").unwrap_or_else(|_| "pnpm".to_owned());
	let steps: [(&Path, &[&str]); 2] = [
		(
			&workspace,
			&[
				"install",
				"--frozen-lockfile",
				"--prefer-offline",
				"--silent",
			],
		),
		(&workspace.join(package), &["run", "--silent", "build"]),
	];
	for (directory, arguments) in steps {
		let status = Command::new(&pnpm)
			.current_dir(directory)
			.args(arguments)
			.status()
			.map_err(|error| format!("pnpm could not be started: {error}"))?;
		if !status.success() {
			return Err(format!(
				"`pnpm {}` failed in {}: {status}",
				arguments.join(" "),
				directory.display()
			));
		}
	}
	Ok(workspace.join(package).join("dist").join(artefact))
}

/// Runs `cargo build` for `package` and returns the path of the component it writes.
fn run_cargo_build(package: &str, artefact: &str) -> Result<PathBuf, String> {
	let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
	let target = repository
		.join("target/components")
		.join(package.replace('/', "-"));
	let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()))
		.current_dir(repository.join(package))
		.args([
			"build",
			"--quiet",
			"--release",
			"--target",
			"wasm32-wasip3",
			"--target-dir",
		])
		.arg(&target)
		.status()
		.map_err(|error| format!("cargo could not be started: {error}"))?;
	if !status.success() {
		return Err(format!(
			"the component in {package} failed to build: {status}"
		));
	}
	Ok(target.join("wasm32-wasip3/release").join(artefact))
}

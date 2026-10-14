// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! The base and the prefixes in effect in a document, and resolving Internationalized
//! Resource Identifier (IRI) references against them.
//!
//! A document writes an IRI in one of three ways: absolute, relative to the base in effect,
//! or as a prefixed name. [`Namespaces`] holds the base and the prefixes at the current
//! point in a document and turns each reference into an absolute IRI. Resolution follows
//! RFC 3986, through the `oxiri` crate.

use std::collections::HashMap;

use oxiri::Iri;

use crate::interpretation::{ErrorKind, InvalidBase, Problem};
use crate::written::IriReference;

/// The base and the prefixes in effect at the current point in a document.
#[derive(Debug)]
pub struct Namespaces {
	/// The base relative references resolve against, if one is in effect.
	base: Option<Iri<String>>,
	/// Each declared prefix and the absolute IRI it expands to. A later declaration of the
	/// same prefix replaces the earlier one.
	prefixes: HashMap<String, String>,
}

impl Namespaces {
	/// Starts a document with `base` in effect and no prefixes declared.
	pub fn new(base: Option<&str>) -> Result<Self, InvalidBase> {
		let base = base
			.map(|base| Iri::parse(base.to_owned()))
			.transpose()
			.map_err(|error| InvalidBase {
				message: error.to_string(),
			})?;
		Ok(Self {
			base,
			prefixes: HashMap::new(),
		})
	}

	/// Turns a reference into an absolute IRI, as a named node.
	///
	/// An absolute reference is checked but used exactly as written: resolving it would
	/// remove dot segments, changing IRIs that N-Triples documents and JSON-LD parsers have
	/// already settled.
	pub fn resolve(&self, reference: &IriReference) -> Result<oxrdf::NamedNode, Problem> {
		let iri = match reference {
			IriReference::Absolute(iri) => iri.clone(),
			IriReference::Relative(relative) => self.resolve_relative(relative)?,
			IriReference::Prefixed { prefix, local } => self.expand(prefix, local)?,
		};
		oxrdf::NamedNode::new(iri.as_str()).map_err(|error| Problem {
			kind: ErrorKind::InvalidIri,
			message: format!("<{iri}> is not a valid absolute IRI: {error}"),
		})
	}

	/// Declares `prefix`, replacing any earlier declaration of it. A relative namespace is
	/// resolved now, against the base in effect at the declaration.
	pub fn declare_prefix(
		&mut self,
		prefix: &str,
		namespace: &IriReference,
	) -> Result<(), Problem> {
		let namespace = self.resolve(namespace)?;
		self.prefixes
			.insert(prefix.to_owned(), namespace.into_string());
		Ok(())
	}

	/// Makes `reference` the base for the rest of the document. A relative reference is
	/// resolved against the base in effect before it.
	pub fn declare_base(&mut self, reference: &IriReference) -> Result<(), Problem> {
		let base = self.resolve(reference)?;
		let base = Iri::parse(base.into_string()).map_err(|error| Problem {
			kind: ErrorKind::InvalidIri,
			message: format!("the base is not a valid absolute IRI: {error}"),
		})?;
		self.base = Some(base);
		Ok(())
	}

	/// Resolves a relative reference against the base in effect.
	fn resolve_relative(&self, relative: &str) -> Result<String, Problem> {
		let base = self.base.as_ref().ok_or_else(|| Problem {
			kind: ErrorKind::MissingBase,
			message: format!("<{relative}> is relative, and no base is in effect to resolve it"),
		})?;
		base.resolve(relative)
			.map(Iri::into_inner)
			.map_err(|error| Problem {
				kind: ErrorKind::InvalidIri,
				message: format!("<{relative}> cannot be resolved against <{base}>: {error}"),
			})
	}

	/// Expands a prefixed name using the latest declaration of its prefix.
	fn expand(&self, prefix: &str, local: &str) -> Result<String, Problem> {
		let namespace = self.prefixes.get(prefix).ok_or_else(|| Problem {
			kind: ErrorKind::UndefinedPrefix,
			message: format!("the prefix `{prefix}:` in `{prefix}:{local}` has not been declared"),
		})?;
		Ok(format!("{namespace}{local}"))
	}
}

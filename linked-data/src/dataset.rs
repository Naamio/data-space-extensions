// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! The dataset the host stores interpreted quads in.

use std::error::Error;
use std::fmt;

/// A quad refused because the dataset already holds as many quads as its limit allows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LimitReached;

impl fmt::Display for LimitReached {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter.write_str("the dataset already holds as many quads as its limit allows")
	}
}

impl Error for LimitReached {}

/// A Resource Description Framework (RDF) dataset: a set of quads, each in the default
/// graph or a named graph, with an optional limit on how many it may hold.
#[derive(Debug, Default)]
pub struct Dataset {
	/// The quads the dataset holds.
	quads: oxrdf::Dataset,
	/// The most quads the dataset may hold, if the host has set a limit.
	quad_limit: Option<usize>,
}

impl Dataset {
	/// Creates an empty dataset with no limit on its size.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Creates an empty dataset that refuses any quad beyond the first `limit`.
	///
	/// A host gives the dataset for each document a limit, so that one document cannot
	/// exhaust the memory available to everyone else.
	#[must_use]
	pub fn with_quad_limit(limit: usize) -> Self {
		Self {
			quad_limit: Some(limit),
			..Self::default()
		}
	}

	/// Adds a quad, and returns whether it was new.
	///
	/// A quad the dataset already holds changes nothing and returns `false`, even when the
	/// dataset is full, because it does not make the dataset larger.
	///
	/// # Errors
	///
	/// Returns [`LimitReached`] if the quad is new and the dataset already holds as many
	/// quads as its limit allows. The dataset is unchanged.
	pub fn insert(&mut self, quad: &oxrdf::Quad) -> Result<bool, LimitReached> {
		if self.quads.contains(quad) {
			return Ok(false);
		}
		if self
			.quad_limit
			.is_some_and(|limit| self.quads.len() >= limit)
		{
			return Err(LimitReached);
		}
		Ok(self.quads.insert(quad))
	}

	/// Whether the dataset holds `quad`.
	#[must_use]
	pub fn contains(&self, quad: &oxrdf::Quad) -> bool {
		self.quads.contains(quad)
	}

	/// The number of quads the dataset holds.
	#[must_use]
	pub fn len(&self) -> usize {
		self.quads.len()
	}

	/// Whether the dataset holds no quads.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.quads.is_empty()
	}

	/// Every quad the dataset holds, in no particular order.
	pub fn quads(&self) -> impl Iterator<Item = oxrdf::QuadRef<'_>> {
		self.quads.iter()
	}
}

// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! The host's ends of the stream and future a parser component returns.
//!
//! Wasmtime delivers what a component writes by calling a consumer the host attaches to
//! each stream or future. [`StatementConsumer`] hands each batch of statements to the parse
//! in progress, and tells the component to stop writing once the host wants no more.
//! [`OutcomeConsumer`] passes the component's final outcome to the code waiting for it.

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use tokio::sync::oneshot;
use wasmtime::StoreContextMut;
use wasmtime::component::{FutureConsumer, Source, StreamConsumer, StreamResult};

use crate::bindings::exports::linked_data::rdf::parser::ParseError;
use crate::bindings::linked_data::rdf::terms::LocatedStatement;
use crate::ingestion::Ingestion;
use crate::statements::located_statement;

/// The ingestion a parse is writing into, shared between the statement consumer, which
/// Wasmtime owns while the stream is open, and the parse, which takes it back at the end.
/// `None` once the parse has taken it back.
pub type SharedIngestion = Arc<Mutex<Option<Ingestion>>>;

/// The outcome a parser component reports through its future.
pub type ParseOutcome = Result<(), ParseError>;

/// Reads the statements a parser component writes and hands them to the ingestion.
#[derive(Debug)]
pub struct StatementConsumer {
	/// The ingestion the statements go into.
	ingestion: SharedIngestion,
}

impl StatementConsumer {
	/// Creates a consumer that hands statements to `ingestion`.
	pub const fn new(ingestion: SharedIngestion) -> Self {
		Self { ingestion }
	}
}

impl<D> StreamConsumer<D> for StatementConsumer {
	type Item = LocatedStatement;

	/// Takes every statement the component has written so far, and passes each one to the
	/// ingestion in order. When the ingestion refuses one, the rest of the batch is
	/// discarded and the consumer reports the stream as dropped, which ends the
	/// component's writes; the contract asks a parser to stop reading its document when
	/// that happens.
	fn poll_consume(
		self: Pin<&mut Self>,
		_context: &mut Context<'_>,
		mut store: StoreContextMut<D>,
		mut source: Source<'_, Self::Item>,
		_finish: bool,
	) -> Poll<wasmtime::Result<StreamResult>> {
		let mut batch: Vec<LocatedStatement> = Vec::with_capacity(source.remaining(&mut store));
		source.read(&mut store, &mut batch)?;

		// The lock is held only while the batch is taken, so the parse can take the
		// ingestion back as soon as the stream ends.
		let wants_more = {
			let mut shared = self.ingestion.lock().map_err(|_| {
				wasmtime::Error::msg("the ingestion was left inconsistent by a panic")
			})?;
			shared.as_mut().is_some_and(|ingestion| {
				batch
					.into_iter()
					.all(|statement| ingestion.take(&located_statement(statement)))
			})
		};

		Poll::Ready(Ok(if wants_more {
			StreamResult::Completed
		} else {
			StreamResult::Dropped
		}))
	}
}

/// Receives the outcome a parser component writes to its future and sends it to the parse
/// waiting for it.
#[derive(Debug)]
pub struct OutcomeConsumer {
	/// The sending end of the channel the outcome goes through. `None` once it has been
	/// sent.
	sender: Option<oneshot::Sender<ParseOutcome>>,
}

impl OutcomeConsumer {
	/// Creates a consumer that sends the outcome through `sender`.
	pub const fn new(sender: oneshot::Sender<ParseOutcome>) -> Self {
		Self {
			sender: Some(sender),
		}
	}
}

impl<D> FutureConsumer<D> for OutcomeConsumer {
	type Item = ParseOutcome;

	/// Reads the outcome and sends it on. If the parse has stopped waiting, the outcome has
	/// no one to go to and is discarded.
	fn poll_consume(
		mut self: Pin<&mut Self>,
		_context: &mut Context<'_>,
		mut store: StoreContextMut<D>,
		mut source: Source<'_, Self::Item>,
		_finish: bool,
	) -> Poll<wasmtime::Result<()>> {
		let mut outcome: Option<ParseOutcome> = None;
		source.read(&mut store, &mut outcome)?;
		if let (Some(outcome), Some(sender)) = (outcome, self.sender.take()) {
			// Sending fails only when the parse has stopped waiting, as described above.
			let _ = sender.send(outcome);
		}
		Poll::Ready(Ok(()))
	}
}

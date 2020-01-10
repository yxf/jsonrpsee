// Copyright 2019 Parity Technologies (UK) Ltd.
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use derive_more::*;
use err_derive::*;
use tokio_util::codec::LinesCodecError;
use futures::channel::mpsc::SendError;

/// Error that can happen during a request.
#[derive(Debug, From, Error)]
pub enum Error {
	/// Error while serializing the request.
	#[error(display = "error while serializing the request")]
	Serialization(#[error(cause)] serde_json::error::Error),

	/// Response given by the server failed to decode as UTF-8.
	#[error(display = "response body is not UTF-8")]
	IO(#[error(cause)] LinesCodecError),

	/// can't send requests to the background task
	#[error(display = "can't send to the background tasks")]
	SendError(#[error(cause)] SendError),
}
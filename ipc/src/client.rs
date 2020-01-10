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

use jsonrpsee_core::{
	common,
	client::{TransportClient, RawClientEvent}
};
use jsonrpsee_core::common::{Response, Request};
use std::{pin::Pin, io};
use crate::error::Error;
use futures::{future, stream, Future, Stream, AsyncRead, AsyncWrite, StreamExt, FutureExt, AsyncWriteExt};
use parity_tokio_ipc::{Endpoint, Incoming};
use tokio::{
	io::WriteHalf,
	net::UnixStream,
	prelude::*
};
use tokio_util::codec::{Framed, LinesCodec};
use futures::{
	channel::{mpsc, oneshot},
	SinkExt
};
use tokio::prelude::*;
use serde::export::PhantomData;
use std::collections::HashMap;
use std::path::Path;
use jsonrpsee_core::client::RawClientError::RequestError;
use futures::stream::SplitStream;

/// Client for IPC transports
pub struct IpcTransportClient {
	stream: ResponseStream,
	request_sink: mpsc::Sender<Request>
}

type ResponseStream = SplitStream<Framed<UnixStream, LinesCodec>>;

impl IpcTransportClient {
	pub async fn new<P: 'static + AsRef<Path> + Send>(path: P) -> io::Result<Self> {
		let (sender, future) = oneshot::channel();
		let (request_sink, request_stream) = mpsc::channel(100);

		std::thread::Builder::new()
			.name("jsonrpsee-ipc-client".into())
			.spawn(move || {
				background_thread(path, request_stream, sender)
			})
			.unwrap();

		let stream = future.await.expect("sender is never dropped; qed")?;

		Ok(IpcTransportClient {
			stream,
			request_sink
		})
	}
}

impl TransportClient for IpcTransportClient {
	type Error = Error;

	fn send_request<'s>(
		&'s mut self,
		request: common::Request,
	) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 's>> {
		async move {
			Ok(self.request_sink.send(request).await?)
		}.boxed()
	}

	fn next_response<'s>(
		&'s mut self,
	) -> Pin<Box<dyn Future<Output = Result<common::Response, Error>> + Send + 's>> {
		async move {
			loop {
				match self.stream.next().await {
					Some(Ok(string)) => return Ok(serde_json::from_str(&string)?),
					Some(Err(e)) => {
						// TODO: what to do
						return Err(e.into())
					}
					None => continue
				}
			}
		}.boxed()
	}
}

/// Function that runs in a background thread.
fn background_thread<P: AsRef<Path>>(
	path: P,
	mut requests_stream: mpsc::Receiver<common::Request>,
	mut sender: oneshot::Sender<io::Result<ResponseStream>>
) {
	let mut runtime = match tokio::runtime::Builder::new()
		.basic_scheduler()
		.enable_all()
		.build()
		{
			Ok(r) => r,
			Err(err) => {
				return sender.send(Err(err)).expect("receiving end isn't dropped; qed")
			}
		};

	// Running until the channel has been closed, and all requests have been completed.
	runtime.block_on(async move {
		// Collection of futures that process ongoing requests.
		let mut sink = match UnixStream::connect(path).await {
			Ok(io) => {
				let (sink, stream) = Framed::new(io, LinesCodec::new()).split();
				sender.send(Ok(stream)).expect("receiving end isn't dropped; qed");
				sink
			},
			Err(e) => {
				return sender.send(Err(e)).expect("receiving end isn't dropped; qed")
			}
		};

		loop {
			let rq = match requests_stream.next().await {
				// We received a request from the foreground.
				Some(rq) => rq,
				// The channel with the foreground has closed.
				None => break,
			};

			let request = serde_json::to_string(&rq).expect("shouldn't fail");
			let _ = sink.send(request).await;
		}
	});
}

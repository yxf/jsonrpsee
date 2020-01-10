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
	server::raw::{TransportServer, TransportServerEvent},
};
use jsonrpsee_core::common::Response;
use std::{pin::Pin, io};

use futures::{Future, Stream, AsyncRead, AsyncWrite, StreamExt, FutureExt, AsyncWriteExt};
use parity_tokio_ipc::{Endpoint, Incoming};
use tokio::{
	io::WriteHalf,
	net::UnixStream,
	prelude::*
};
use serde::export::PhantomData;
use std::collections::HashMap;

pub struct IpcTransportServer {
	next_request_id: u64,
	endpoint: Endpoint,
	connections: HashMap<u64, WriteHalf<UnixStream>>,
}

impl IpcTransportServer {
	// TODO: bind_with_acl

	pub fn bind(path: &str) -> io::Result<Self> {
		let mut endpoint = Endpoint::new(path.to_owned());
		Ok(Self {
			endpoint,
			connections: HashMap::new(),
			next_request_id: 0,
		})
	}
}

impl TransportServer for IpcTransportServer {
	type RequestId = u64;

	fn next_request<'a>(
		&'a mut self
	) -> Pin<Box<dyn Future<Output=TransportServerEvent<Self::RequestId>> + Send + 'a>> {
		async move {
			loop {
				let socket = match self.endpoint.listener_mut().accept().await {
					Ok((socket, _)) => socket,
					Err(e) => {
						println!("whoops! {}", e);
						continue
					}
				};

				let (mut reader, writer) = tokio::io::split(socket);

				let mut buffer = Vec::new();
				// TODO: read in chunks of 100kb in order to enforce a json size limit.
				match reader.read_to_end(&mut buffer).await {
					Ok(_size) => {},
					Err(e) => {
						println!("io error: {}", e);
						continue
					}
				};

				match serde_json::from_slice(&buffer[..]) {
					Ok(request) => {
						let id = match self.next_request_id.checked_add(1) {
							Some(id) => id,
							None => unimplemented!()
						};

						self.connections.insert(id, writer);

						return TransportServerEvent::Request {
							request,
							id,
						}
					},
					Err(e) => {
						// TODO: yeah we should probably send back this error with the writer.
						println!("whoops! {}", e);
						continue
					}
				}
			}
		}.boxed()
	}

	fn finish<'a>(
		&'a mut self,
		request_id: &'a Self::RequestId,
		response: Option<&'a Response>
	) -> Pin<Box<dyn Future<Output=Result<(), ()>> + Send + 'a>> {
		async move {
			let mut writer = match self.connections.remove(request_id) {
				Some(w) => w,
				None => return Err(())
			};

			match response.map(|r| serde_json::to_vec(r)) {
				Some(Ok(bytes)) => {
					writer.write_all(&bytes[..]).await.map_err(|_| ())?;
					Ok(())
				}
				Some(Err(_)) => panic!(), // TODO: no
				None => {
					// TODO: not entirely sure why response is Option.
					panic!("as well")
				}
			}
		}.boxed()
	}

	fn supports_resuming<'a>(
		&self,
		request_id: &Self::RequestId
	) -> Result<bool, ()> {
		if self.connections.contains_key(request_id) {
			Ok(false)
		} else {
			Err(())
		}
	}

	fn send<'a>(
		&'a mut self,
		request_id: &'a Self::RequestId,
		response: &'a Response
	) -> Pin<Box<dyn Future<Output=Result<(), ()>> + Send + 'a>> {
		async move { Err(()) }.boxed()
	}
}
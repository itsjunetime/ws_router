use warp::ws::{
	WebSocket,
	Message,
};
use crate::sockets::SocketType;
use futures_util::{
	stream::SplitSink,
	SinkExt
};
use std::sync::Arc;

pub struct Connection {
	pub sender: SplitSink<WebSocket, Message>,
	pub sock_type: SocketType,
}

use crate::sockets::SocketType;
use futures_util::stream::SplitSink;
use warp::ws::{Message, WebSocket};

pub struct Connection {
	pub sender: SplitSink<WebSocket, Message>,
	pub sock_type: SocketType,
	pub uuid: String,
}

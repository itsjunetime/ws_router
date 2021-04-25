use warp::{
	Reply,
	Rejection,
	reject,
	ws::WebSocket
};
use crate::{
	Registrations,
	sockets::*,
	register::RegistrationType
};
use futures_util::StreamExt;

pub struct Socket;

impl Socket {
	pub async fn connect_handler(
		ws: warp::ws::Ws, req: SocketRequest, registrations: Registrations
	) -> Result<impl Reply, Rejection> {

		let regists = registrations.read().await;

		let reg_type = if let Some(reg) = regists.get(&req.id) {
			if !reg.verify_key(&req.key) {
				Err(reject::custom(Rejections::IncorrectKey))
			} else {

				match reg.reg_type {
					RegistrationType::HostClient if req.sock_type.as_str() != "client" &&
						req.sock_type.as_str() != "host" => Err(reject::custom(Rejections::InvalidSockType)),
					_ => Ok(reg.reg_type)
				}

			}
		} else {
			Err(reject::not_found())
		}?;

		let sock_type = match reg_type {
			RegistrationType::Lobby => SocketType::Socket,
			RegistrationType::HostClient => match req.sock_type.as_str() {
				"host" => SocketType::Host,
				_ => SocketType::Client
			}
		};

		Ok(ws.on_upgrade(move |socket|
			Socket::spawn_forwarding(socket, req.id.to_owned(), registrations, sock_type)
		))
	}

	pub async fn spawn_forwarding(
		ws: WebSocket,
		id: String,
		registrations: Registrations,
		sock_type: SocketType
	) {
		let (ws_sender, ws_receiver) = ws.split();

		let mut registers = registrations.write().await;

		if let Some(reg) = registers.get_mut(&id) {
			reg.add_connection(ws_sender, sock_type).await;
			reg.spawn_sending(ws_receiver, sock_type);
		}
	}
}

#[derive(Eq, PartialEq, Copy, Clone)]
pub enum SocketType {
	Socket,
	Host,
	Client,
}

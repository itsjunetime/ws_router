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
				eprintln!("\x1b[31;1m✗\x1b[0m Failed to verify key ({} against hash {})", req.key, reg.key);
				Err(reject::custom(Rejections::IncorrectKey))
			} else {

				println!("\x1b[34;1m=>\x1b[0m Key verified successfully");

				match reg.reg_type {
					RegistrationType::HostClient => {
						match req.sock_type {
							Some(ref st) => if st.as_str() != "host" && st.as_str() != "client" {
								Err(reject::custom(Rejections::InvalidSockType))
							} else {
								Ok(reg.reg_type)
							},
							None => Err(reject::custom(Rejections::InvalidSockType)),
						}
					},
					_ => Ok(reg.reg_type)
				}

			}
		} else {
			eprintln!("\x1b[31;1m✗\x1b[0m Connection attemped to access registration with id {}, which does not exist.", req.id);
			Err(reject::not_found())
		}?;

		println!("\x1b[34;1m=>\x1b[0m Got reg_type {:?}", reg_type);

		let sock_type = match reg_type {
			RegistrationType::Lobby => Ok(SocketType::Socket),
			RegistrationType::HostClient => match req.sock_type {
				Some(ref st) => match st.as_str() {
					"host" => Ok(SocketType::Host),
					_ => Ok(SocketType::Client),
				},
				None => Err(reject::custom(Rejections::InvalidSockType))
			}
		}?;

		println!("\x1b[34;1m=>\x1b[0m Got sock_type {:?}, upgrading...", sock_type);

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

		let reg_clone = registrations.clone();
		let mut registers = registrations.write().await;

		if let Some(reg) = registers.get_mut(&id) {
			let uuid = reg.add_connection(ws_sender, sock_type).await;
			reg.spawn_sending(ws_receiver, sock_type, reg_clone, uuid, id);
		}
	}
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum SocketType {
	Socket,
	Host,
	Client,
}

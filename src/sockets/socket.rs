use warp::{
	Reply,
	Rejection,
	reject,
	ws::WebSocket
};
use crate::{
	Registrations,
	sockets::*,
	register::RegistrationType,
	config::*,
	err, log, log_vbs
};
use futures_util::StreamExt;

pub struct Socket;

impl Socket {
	pub async fn connect_handler(
		ws: warp::ws::Ws, req: SocketRequest, registrations: Registrations
	) -> Result<impl Reply, Rejection> {
		let (out, _) = Config::out_and_vbs().await;

		log!(out, Color::Blue, "Websocket attempting to connect to registration with id \x1b[1m{}\x1b[0m", req.id);

		let regists = registrations.read().await;

		let reg_type = if let Some(reg) = regists.get(&req.id) {
			if !reg.verify_key(&req.key).await {
				err!(out, "Failed to verify key ('{}' against hash '{}')", req.key, reg.key);
				Err(reject::custom(Rejections::IncorrectKey))
			} else {
				log!(out, Color::Blue, "Key verified successfully");

				match reg.reg_type {
					RegistrationType::HostClient => {
						match req.sock_type {
							Some(ref st) => {
								// remove potential trailing slashes 'cause that's what the 
								// rust URL crate adds
								let st_rem = st.as_str().replace("/", "");

								if st_rem != "host" && st_rem != "client" {
									err!(out, "Rejecting because st is '{}', which is not allowed", st);
									Err(reject::custom(Rejections::InvalidSockType))
								} else {
									Ok(reg.reg_type)
								}
							},
							None => {
								err!(out, "Rejecting because req.sock_type is none");
								Err(reject::custom(Rejections::InvalidSockType))
							}
						}
					},
					_ => Ok(reg.reg_type)
				}

			}
		} else {
			err!(out, "Request attempted to access registration with id {}, which does not exist", req.id);
			Err(reject::not_found())
		}?;

		log!(out, Color::Blue, "Got reg_type {:?}", reg_type);

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

		log!(out, Color::Blue, "Got sock_type {:?}, upgrading...", sock_type);

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
		let (out, vbs) = Config::out_and_vbs().await;

		log_vbs!(vbs, out, "Spawning forwarding for socket with id {} and sock_type {:?}", id, sock_type);

		let (ws_sender, ws_receiver) = ws.split();

		let reg_clone = registrations.clone();
		let mut registers = registrations.write().await;

		if let Some(reg) = registers.get_mut(&id) {
			let uuid = reg.add_connection(ws_sender, sock_type).await;
			reg.spawn_sending(ws_receiver, sock_type, reg_clone, uuid, id);
		}

		log_vbs!(vbs, out, "Successfully added connection and spawned forwarding");
	}
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum SocketType {
	Socket,
	Host,
	Client,
}

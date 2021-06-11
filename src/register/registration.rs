use crate::{
	Registrations,
	register::*,
	sockets::SocketType,
	connections::Connection,
};
use warp::{
	Reply,
	Rejection,
	reject,
	ws::{
		WebSocket,
		Message
	}
};
use std::{
	result::Result,
	vec::Vec,
	sync::Arc,
	collections::hash_map::Entry
};
use uuid::Uuid;
use futures_util::{
	stream::{
		SplitSink,
		SplitStream,
	},
	SinkExt,
	StreamExt
};
use futures_locks::RwLock;
use crate::{
	config::*,
	CONFIG,
};

pub struct Registration {
	pub uuid: String,
	pub key: String,
	pub host_key: String,
	pub reg_type: RegistrationType,
	pub connections: Arc<RwLock<Vec<Connection>>>,
	pub destroy: Arc<RwLock<bool>>,
}

impl Registration {
	pub async fn new(
		unhashed_key: &str,
		unhashed_host_key: &str,
		reg_type: RegistrationType,
		registrations: Registrations
	) -> Result<Registration, argon2::Error> {
		let conf = CONFIG.read().await;
		let out = !conf.quiet;

		Config::log("Attempting to create new registration..", out, Color::Green);

		let config = argon2::Config::default();

		let key = argon2::hash_encoded(
			unhashed_key.as_bytes(),
			conf.secret_key.as_bytes(),
			&config
		)?;

		let host_key = argon2::hash_encoded(
			unhashed_host_key.as_bytes(),
			conf.secret_key.as_bytes(),
			&config
		)?;

		drop(conf);

		let uuid = Uuid::new_v4().to_simple().to_string();

		let destroy = Arc::new(RwLock::new(false));

		let mut uuid_str = uuid[..8].to_owned();

		let reg = registrations.read().await;

		while reg.get(&uuid_str).is_some() {
			let uuid = Uuid::new_v4().to_simple().to_string();
			uuid_str = uuid[..8].to_owned();
		}

		Config::log(
			&format!("Created new registration with uuid '{}'", uuid),
			out,
			Color::Green
		);

		Ok(Registration {
			uuid: uuid_str.to_owned(),
			connections: Arc::new(RwLock::new(Vec::new())),
			key,
			host_key,
			reg_type,
			destroy
		})
	}

	pub async fn new_handler(
		body: RegisterRequest, rgs: Registrations
	) -> Result<impl Reply, Rejection> {
		let conf = CONFIG.read().await;
		let out = !conf.quiet;
		drop(conf);

		Config::log("Received request for new registration...", out, Color::Green);

		let reg_type = match body.reg_type.as_str() {
			"hostclient" => Some(RegistrationType::HostClient),
			"lobby" => Some(RegistrationType::Lobby),
			_ => None
		};

		if let Some(reg) = reg_type {
			let reg_clone = rgs.clone();
			let new_register = Registration::new(
				&body.key,
				&body.host_key,
				reg,
				reg_clone
			).await;

			match new_register {
				Ok(new_reg) => {
					let uuid = new_reg.uuid.to_owned();

					let mut regs = rgs.write().await;

					regs.insert(uuid.to_owned(), new_reg);
					Config::log(&format!("Saved new registration with key '{}'", uuid), out, Color::Green);
					Ok(uuid)
				},
				Err(_) => {
					Config::err("One or more of the keys in the registration request is unhashable", out);
					Err(reject::custom(Rejections::UnhashableKey))
				}
			}
		} else {
			Config::err("Registration type missing in registration request", out);
			Err(reject::custom(Rejections::MissingRegistrationType))
		}
	}

	pub async fn remove_handler(
		body: RemoveRequest, rgs: Registrations
	) -> Result<impl Reply, Rejection> {
		let conf = CONFIG.read().await;
		let out = !conf.quiet;
		drop(conf);

		Config::log(&format!("Received request to remove registration with uuid '{}'", body.id), out, Color::Yellow);

		let mut regs = rgs.write().await;

		let _ = if let Some(reg) = regs.get(&body.id) {
			let key_ver = reg.verify_key(&body.key);
			let host_ver = reg.verify_host_key(&body.host_key);

			if key_ver.await && host_ver.await {
				Config::log(&format!("Verified keys. Removing registration with key '{}'", body.id), out, Color::Yellow);

				let mut destroy = reg.destroy.write().await;
				*destroy = true;
				drop(destroy);

				Ok(())
			} else {
				Config::err("Failed to verify keys. Not removing registration", out);
				Err(reject::custom(Rejections::InvalidKey))
			}
		} else {
			Config::err("Registration not found.", out);
			Err(reject::not_found())
		}?;

		if let Entry::Occupied(reg) = regs.entry(body.id) {
			reg.remove_entry();
			Ok("")
		} else {
			// This should be unreachable!(), since we already verified that it exists in
			// the hashmap before getting here. However, we're not gonna panic 'cause this
			// service needs to be, like, panic-proof
			Err(reject::not_found())
		}
	}

	pub async fn verify_key(&self, key: &str) -> bool {
		let conf = CONFIG.read().await;
		let out = !conf.quiet;
		drop(conf);

		match argon2::verify_encoded(&self.key, key.as_bytes()) {
			Err(_) => {
				Config::err(&format!("Failed to verify key '{}' against hash '{}'", key, self.key), out);
				false
			},
			Ok(val) => val
		}

	}

	pub async fn verify_host_key(&self, key: &str) -> bool {
		let conf = CONFIG.read().await;
		let out = !conf.quiet;
		drop(conf);

		match argon2::verify_encoded(&self.host_key, key.as_bytes()) {
				Err(_) => {
					Config::err(&format!("Failed to verify host key '{}' against hash '{}'", key, self.host_key), out);
					false
				},
				Ok(val) => val
			}
	}

	pub async fn add_connection(
		&mut self,
		sender: SplitSink<WebSocket, Message>,
		sock_type: SocketType,
	) -> String {
		let mut buf = Uuid::encode_buffer();
		let uuid = Uuid::new_v4().to_simple()
			.encode_lower(&mut buf)
			.to_owned();

		let uuid_clone = uuid.to_owned();

		let mut con = self.connections.write().await;

		con.push(
			Connection {
				sender,
				sock_type,
				uuid
			}
		);

		uuid_clone
	}

	pub fn spawn_sending(
		&self,
		receiver: SplitStream<WebSocket>,
		sock_type: SocketType,
		registrations: Registrations,
		con_uuid: String,
		reg_uuid: String
	) {
		let conn = self.connections.clone();
		let dest = self.destroy.clone();

		tokio::spawn(async move {
			let mut mut_rec = receiver;

			let conf = CONFIG.read().await;
			let out = !conf.quiet;
			drop(conf);

			Config::log("Successfully upgraded. Awaiting messages...", out, Color::Yellow);

			while let Some(res) = mut_rec.next().await {
				match res {
					Ok(msg) => {
						let should_destroy = dest.read().await;
						if *should_destroy {
							break;
						}

						let mut conns = conn.write().await;

						for con in conns.iter_mut()
							.filter(|c|
								c.sock_type == match sock_type {
									SocketType::Socket => SocketType::Socket,
									SocketType::Client => SocketType::Host,
									SocketType::Host => SocketType::Client
								}
								&&
								c.uuid != con_uuid
							) {

							let new_msg = msg.clone();

							if let Err(err) = con.sender.send(new_msg).await {
								Config::err(&format!("Failed to send message: {:?}", err), out);
							}
						}
					},
					Err(err) => Config::err(&format!("Warp error: {}", err), out),
				}
			}

			let mut conns = conn.write().await;

			if let Some(m_conn) = conns.iter().position(|c| c.uuid == con_uuid) {
				let sink = conns.remove(m_conn);

				if let Ok(ws) = mut_rec.reunite(sink.sender) {
					match ws.close().await {
						Err(err) => Config::err(&format!("Failed to close websocket nicely: {}", err), out),
						Ok(_) => Config::log("Successfully closed websocket nicely", out, Color::Blue),
					}
				} else {
					Config::err("Found matching sender but failed to reunite sender and receiver", out);
				}
			} else {
				Config::err("Failed to find matching connection to remove", out);
			}

			if conns.len() == 0 {
				Config::log("No connections remaining. Removing registration...", out, Color::Blue);

				let mut regs = registrations.write().await;

				if let Entry::Occupied(reg) = regs.entry(reg_uuid) {
					reg.remove_entry();
				}
			}
		});
	}
}

#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub enum RegistrationType {
	HostClient,
	Lobby
}

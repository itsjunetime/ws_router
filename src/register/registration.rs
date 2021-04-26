use argonautica::{Hasher, Verifier};
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

pub struct Registration {
	pub uuid: String,
	pub key: String,
	pub host_key: String,
	pub reg_type: RegistrationType,
	pub connections: Arc<RwLock<Vec<Connection>>>,
}

impl Registration {
	pub fn new(
		unhashed_key: &str,
		unhashed_host_key: &str,
		reg_type: RegistrationType
	) -> Result<Registration, argonautica::Error> {
		println!("\x1b[32;1m=>\x1b[0m Attempting to create new registration...");

		let mut hasher = Hasher::default();

		let key = hasher.with_password(unhashed_key)
			.with_secret_key(crate::TEST_KEY)
			.hash()?;

		let host_key = hasher.with_password(unhashed_host_key)
			.with_secret_key(crate::TEST_KEY)
			.hash()?;

		let mut buf = Uuid::encode_buffer();
		let uuid = Uuid::new_v4().to_simple().encode_lower(&mut buf);

		println!("\x1b[32;1m=>\x1b[0m Created new registration with uuid '{}'", uuid);

		Ok(Registration {
			uuid: uuid.to_owned(),
			connections: Arc::new(RwLock::new(Vec::new())),
			key,
			host_key,
			reg_type
		})
	}

	pub async fn new_handler(
		body: RegisterRequest, rgs: Registrations
	) -> Result<impl Reply, Rejection> {
		println!("\x1b[32;1m=>\x1b[0m Received request for new registration...");

		let mut regs = rgs.write().await;

		let reg_type = match body.reg_type.as_str() {
			"hostclient" => Some(RegistrationType::HostClient),
			"lobby" => Some(RegistrationType::Lobby),
			_ => None
		};

		if let Some(reg) = reg_type {
			let new_register = Registration::new(
				&body.key,
				&body.host_key,
				reg
			);

			match new_register {
				Ok(new_reg) => {
					let uuid = new_reg.uuid.to_owned();
					regs.insert(uuid.to_owned(), new_reg);
					println!("\x1b[32;1m=>\x1b[0m Saved new registration with key '{}'", uuid);
					Ok(uuid)
				},
				Err(_) => {
					eprintln!("\x1b[31;1m✗\x1b[0m One or more of the keys in the registration request is unhashable");
					Err(reject::custom(Rejections::UnhashableKey))
				}
			}
		} else {
			eprintln!("\x1b[31;1m✗\x1b[0m Registration type missing in registration request");
			Err(reject::custom(Rejections::MissingRegistrationType))
		}
	}

	pub fn verify_key(&self, key: &str) -> bool {
		let mut verifier = Verifier::default();

		match verifier.with_hash(self.key.as_str())
			.with_password(key)
			.with_secret_key(crate::TEST_KEY)
			.verify() {
				Err(_) => {
					eprintln!("Failed to verify key {} against hash {}", key, self.key);
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

		tokio::spawn(async move {
			let mut mut_rec = receiver;

			while let Some(Ok(msg)) = mut_rec.next().await {
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
						eprintln!("Failed to send message: {:?}", err);
					}
				}
			}

			println!("\x1b[34;1m=>\x1b[0m Connection to websocket disconnected. Removing from connections...");

			let mut conns = conn.write().await;
			conns.retain(|c| c.uuid != con_uuid);

			if conns.len() == 0 {
				println!("\x1b[34;1m=>\x1b[0m No connections remaining. Removing registration...");

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

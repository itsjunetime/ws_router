use crate::{config::*, CONFIG};
use crate::{
	connections::Connection, err, log, log_vbs, register::*, sockets::SocketType, Registrations,
};
use futures_locks::RwLock;
use futures_util::{
	stream::{SplitSink, SplitStream},
	SinkExt, StreamExt,
};
use std::{collections::hash_map::Entry, result::Result, sync::Arc, vec::Vec, time::Duration};
use uuid::Uuid;
use warp::{
	reject,
	ws::{Message, WebSocket},
	Rejection, Reply,
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
		id_req: Option<String>,
		registrations: Registrations,
	) -> Result<Registration, Rejections> {
		let conf = CONFIG.read().await;
		let (out, vbs, reject) = (!conf.quiet, conf.verbose, conf.reject_no_id);
		let secret_key_bytes = conf.secret_key.as_bytes().to_vec();
		drop(conf);

		log!(
			out,
			Color::Green,
			"Attempting to create new registration..."
		);

		let config = argon2::Config::default();

		let key = argon2::hash_encoded(
			unhashed_key.as_bytes(),
			&secret_key_bytes,
			&config
		).map_err(|_| Rejections::UnhashableKey)?;

		let host_key = argon2::hash_encoded(
			unhashed_host_key.as_bytes(),
			&secret_key_bytes,
			&config,
		).map_err(|_| Rejections::UnhashableKey)?;

		log_vbs!(vbs, out, "Verified keys...");

		let has_id_req = id_req.is_some();

		// we have to make sure that the id they entered is greater than 7 characters
		// so that it doesn't cause a crash when uuid_str is truncated
		let uuid = match id_req {
			Some(id) if id.len() == 8 => id,
			Some(_) if reject => return Err(Rejections::IncorrectLengthID),
			_ => Uuid::new_v4().to_simple().to_string(),
		};

		let destroy = Arc::new(RwLock::new(false));

		let mut uuid_str = uuid[..8].to_owned();

		log_vbs!(
			vbs,
			out,
			"Created shortened uuid \x1b[1m{}\x1b[0m",
			uuid_str
		);

		let reg = registrations.read().await;

		while reg.get(&uuid_str).is_some() {
			if has_id_req && reject {
				return Err(Rejections::InUseID);
			}

			log_vbs!(
				vbs,
				out,
				"The uuid \x1b[1m{}\x1b[0m is already in use. Retrying...",
				uuid_str
			);

			let uuid = Uuid::new_v4().to_simple().to_string();
			uuid_str = uuid[..8].to_owned();
		}

		log!(
			out,
			Color::Green,
			"Created new registration with uuid \x1b[1m{}\x1b[0m",
			uuid_str
		);

		Ok(Registration {
			uuid: uuid_str.to_owned(),
			connections: Arc::new(RwLock::new(Vec::new())),
			key,
			host_key,
			reg_type,
			destroy,
		})
	}

	pub async fn new_handler(
		body: RegisterRequest,
		rgs: Registrations,
	) -> Result<impl Reply, Rejection> {
		let (out, vbs) = Config::out_and_vbs().await;

		log!(
			out,
			Color::Green,
			"Received request for new registration..."
		);

		let reg_type = match body.reg_type.as_str() {
			"hostclient" => Some(RegistrationType::HostClient),
			"lobby" => Some(RegistrationType::Lobby),
			_ => None,
		};

		log_vbs!(vbs, out, "Registration has reg_type {:?}", reg_type);

		if let Some(reg) = reg_type {
			let reg_clone = rgs.clone();
			let new_register =
				Registration::new(&body.key, &body.host_key, reg, body.id_req, reg_clone).await;

			match new_register {
				Ok(new_reg) => {
					log_vbs!(vbs, out, "Successfully created new registration");

					let uuid = new_reg.uuid.to_owned();

					let mut regs = rgs.write().await;

					regs.insert(uuid.to_owned(), new_reg);
					log!(
						out,
						Color::Green,
						"Saved new registration with key \x1b[1m{}\x1b[0m",
						uuid
					);
					Ok(uuid)
				}
				Err(err) => {
					err!(out, "Failed to make new registration: {}", err);
					Err(reject::custom(err))
				}
			}
		} else {
			err!(out, "Registration type missing in registration request");
			Err(reject::custom(Rejections::MissingRegistrationType))
		}
	}

	pub async fn remove_handler(
		body: RemoveRequest,
		rgs: Registrations,
	) -> Result<impl Reply, Rejection> {
		let (out, vbs) = Config::out_and_vbs().await;

		log!(
			out,
			Color::Yellow,
			"Received request to remove registration with id \x1b[1m{}\x1b[0m",
			body.id
		);

		let mut regs = rgs.write().await;

		let _ = if let Some(reg) = regs.get(&body.id) {
			log_vbs!(vbs, out, "Verifying removal request keys...");

			let key_ver = reg.verify_key(&body.key);
			let host_ver = reg.verify_host_key(&body.host_key);

			if key_ver.await && host_ver.await {
				log!(out, Color::Yellow, "Verified keys; removing registration");

				let mut destroy = reg.destroy.write().await;
				*destroy = true;
				drop(destroy);

				Ok(())
			} else {
				err!(out, "Failed to verify keys. Not removing registration");
				Err(reject::custom(Rejections::InvalidKey))
			}
		} else {
			err!(out, "Registration not found");
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
		let (out, vbs) = Config::out_and_vbs().await;

		log_vbs!(vbs, out, "Checking '{}' against '{}'", key, self.key);

		argon2::verify_encoded(&self.key, key.as_bytes())
			.unwrap_or_else(|_| {
				err!(
					out,
					"Failed to verify key '{}' against hash '{}'",
					key,
					self.key
				);
				false
			})
	}

	pub async fn verify_host_key(&self, key: &str) -> bool {
		let (out, vbs) = Config::out_and_vbs().await;

		log_vbs!(vbs, out, "Checking '{}' against '{}", key, self.key);

		argon2::verify_encoded(&self.host_key, key.as_bytes())
			.unwrap_or_else(|_| {
				err!(
					out,
					"Failed to verify host key '{}' against hash '{}'",
					key,
					self.host_key
				);
				false
			})
	}

	pub async fn add_connection(
		&mut self,
		sender: SplitSink<WebSocket, Message>,
		sock_type: SocketType,
	) -> String {
		let (out, vbs) = Config::out_and_vbs().await;

		log_vbs!(vbs, out, "Received request to add connection");

		let uuid = Uuid::new_v4().to_simple().to_string().to_lowercase();
		let uuid_clone = uuid.to_owned();

		log_vbs!(vbs, out, "Generated UUID of \x1b[1m{}\x1b[0m", uuid);

		let mut con = self.connections.write().await;

		con.push(Connection {
			sender,
			sock_type,
			uuid,
		});

		log_vbs!(vbs, out, "Inserted new connection");

		uuid_clone
	}

	pub fn spawn_sending(
		&self,
		mut receiver: SplitStream<WebSocket>,
		sock_type: SocketType,
		registrations: Registrations,
		con_uuid: String,
		reg_uuid: String,
	) {
		let conn = self.connections.clone();
		let dest = self.destroy.clone();

		tokio::spawn(async move {
			let conf = CONFIG.read().await;
			let auto_remove = conf.auto_remove;
			drop(conf);

			let (out, vbs) = Config::out_and_vbs().await;

			log!(
				out,
				Color::Yellow,
				"Successfully upgraded. Awaiting messages..."
			);

			loop {
				// try to get the next message. If there is none in 30 seconds, just send a ping
				// so that the connection is maintained
				if let Ok(next) = tokio::time::timeout(Duration::from_secs(30), receiver.next()).await
				{
					let msg = match next {
						Some(Ok(m)) => {
							if m.is_pong() {
								continue;
							}
							m
						}
						Some(Err(err)) => {
							err!(out, "Warp error when receiving next: {:?}", err);
							continue;
						}
						_ => {
							log_vbs!(
								vbs,
								out,
								"Next message is none for connection {}, breaking...",
								con_uuid
							);
							break;
						}
					};

					// check if this connection should be destroyed, break if so
					if *dest.read().await {
						log_vbs!(
							vbs,
							out,
							"Should destroy connection {}, breaking...",
							con_uuid
						);
						break;
					}

					let mut conns = conn.write().await;

					// find all the other connections that we should send this message to
					for con in conns.iter_mut().filter(|c| {
						c.sock_type
							== match sock_type {
								SocketType::Socket => SocketType::Socket,
								SocketType::Client => SocketType::Host,
								SocketType::Host => SocketType::Client,
							} && c.uuid != con_uuid
					}) {
						// we have to clone it since we're sending it to multiple connections
						let msg_clone = msg.clone();

						log_vbs!(
							vbs,
							out,
							"Attempting to send message to conn id {}",
							con.uuid
						);

						if let Err(err) = con.sender.send(msg_clone).await {
							err!(out, "Failed to send message: {:?}", err);
						}
					}
				} else {
					// if the timeout doesn't return, just send a ping then poll again
					let mut conns = conn.write().await;

					if let Some(con) = conns.iter_mut().find(|c| c.uuid == con_uuid) {
						if let Err(err) = con.sender.send(Message::ping(vec![])).await {
							err!(out, "Failed to send ping: {:?}", err);
						}
					}
				}
			}

			let mut conns = conn.write().await;

			if let Some(m_conn) = conns.iter().position(|c| c.uuid == con_uuid) {
				let sink = conns.remove(m_conn);

				if let Ok(ws) = receiver.reunite(sink.sender) {
					match ws.close().await {
						Err(err) => err!(out, "Failed to close websocket nicely: {}", err),
						Ok(_) => log!(out, Color::Blue, "Successfully closed websocket nicely"),
					}
				} else {
					err!(
						out,
						"Found matching sender but failed to reunite sender and receiver"
					);
				}
			} else {
				err!(out, "Failed to find matching connection to remove");
			}

			let conns_len = conns.len();
			drop(conns);

			if conns_len == 0 && auto_remove {
				log!(
					out,
					Color::Blue,
					"No connections remaining. Removing registration..."
				);

				let mut regs = registrations.write().await;

				if let Entry::Occupied(reg) = regs.entry(reg_uuid) {
					reg.remove_entry();
				}
			} else if auto_remove {
				log_vbs!(
					vbs,
					out,
					"Not removing registration. Remaining connections: {}",
					conns_len
				);
			} else {
				log!(
					out,
					Color::Blue,
					"Remaining connections in this registration: {}",
					conns_len
				);
			}
		});
	}
}

#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub enum RegistrationType {
	HostClient,
	Lobby,
}

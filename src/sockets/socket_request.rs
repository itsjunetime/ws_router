use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
pub struct SocketRequest {
	pub key: String,
	pub id: String,
	pub sock_type: String,
}

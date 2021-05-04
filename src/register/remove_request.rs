use serde_derive::Deserialize;

#[derive(Deserialize)]
pub struct RemoveRequest {
	pub id: String,
	pub key: String,
	pub host_key: String
}

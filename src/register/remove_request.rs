use serde::Deserialize;

#[derive(Deserialize)]
pub struct RemoveRequest {
	pub id: String,
	pub key: String,
	pub host_key: String
}

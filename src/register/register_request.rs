use serde::Deserialize;

#[derive(Deserialize)]
pub struct RegisterRequest {
	pub key: String,
	pub host_key: String,
	pub reg_type: String,
	pub id_req: Option<String>,
}

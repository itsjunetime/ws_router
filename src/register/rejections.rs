use thiserror::Error;

#[derive(Debug, Error)]
pub enum Rejections {
	#[error("Missing the registration type")]
	MissingRegistrationType,
	#[error("Provided key is unhashable")]
	UnhashableKey,
	#[error("Provided key is invalid")]
	InvalidKey,
	#[error("ID is already in use and server is configured to reject requested IDs that are already in use")]
	InUseID,
	#[error("The ID must be exactly 8 characters long")]
	IncorrectLengthID
}

impl warp::reject::Reject for Rejections{}

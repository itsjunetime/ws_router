#[derive(Debug)]
pub enum Rejections {
	MissingRegistrationType,
	UnhashableKey,
	InvalidKey
}

impl warp::reject::Reject for Rejections{}

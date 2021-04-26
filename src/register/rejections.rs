#[derive(Debug)]
pub enum Rejections {
	MissingRegistrationType,
	UnhashableKey,
}

impl warp::reject::Reject for Rejections{}

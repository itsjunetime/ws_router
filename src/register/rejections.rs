#[derive(Debug)]
pub enum Rejections {
	MissingRegistrationType,
	UnlockableHashMap,
	UnhashableKey,
}

impl warp::reject::Reject for Rejections{}

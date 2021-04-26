#[derive(Debug)]
pub enum Rejections {
	IncorrectKey,
	InvalidSockType
}

impl warp::reject::Reject for Rejections{}

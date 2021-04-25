#[derive(Debug)]
pub enum Rejections {
	NonexistentId,
	IncorrectKey,
	InvalidSockType
}

impl warp::reject::Reject for Rejections{}

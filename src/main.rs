use std::{
	sync::Arc,
	collections::HashMap,
	convert::Infallible,
};
use register::Registration;
use warp::Filter;
use sockets::*;
use futures_locks::RwLock;

mod register;
mod sockets;
mod connections;

type Registrations = Arc<RwLock<HashMap<String, Registration>>>;

const TEST_KEY: &str = "This is a test key. Not used for prod.";

#[tokio::main]
async fn main() {
	let registrations: Registrations = Arc::new(RwLock::new(HashMap::new()));

	let register_route = warp::path("register")
		.and(warp::get())
		.and(warp::query())
		.and(with_registrations(registrations.clone()))
		.and_then(Registration::new_handler);

	let connect_route = warp::path("connect")
		.and(warp::ws())
		.and(warp::query())
		.and(with_registrations(registrations.clone()))
		.and_then(Socket::connect_handler);

	let routes = register_route
		.or(connect_route)
		.with(warp::cors().allow_any_origin());

	warp::serve(routes).run(([127, 0, 0, 1], 8741)).await;
}

fn with_registrations(rgs: Registrations) -> impl Filter<Extract = (Registrations,), Error = Infallible> + Clone {
	warp::any().map(move || rgs.clone())
}

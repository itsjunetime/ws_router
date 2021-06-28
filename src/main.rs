use std::{
	sync::Arc,
	collections::HashMap,
	convert::Infallible,
	process::exit,
};
use register::Registration;
use warp::Filter;
use sockets::*;
use futures_locks::RwLock;
use clap::{Arg, App};
use lazy_static::lazy_static;
use config::{
	Config,
	Color
};

mod register;
mod sockets;
mod connections;
mod config;

type Registrations = Arc<RwLock<HashMap<String, Registration>>>;

lazy_static!{
	static ref CONFIG: Arc<RwLock<Config>> = Arc::new(RwLock::new(Config::default()));
}

#[tokio::main]
async fn main() {
	let matches = App::new("warp_router")
		.version("1.0")
		.about("Simple server-side websocket router")
		.arg(Arg::with_name("secret_key")
			.short("k")
			.long("key")
			.help("The secret key to use when hashing and storing passwords")
			.takes_value(true))
		.arg(Arg::with_name("port")
			.short("p")
			.long("port")
			.help("The port to run the router on")
			.takes_value(true))
		.arg(Arg::with_name("quiet")
			.short("q")
			.long("quiet")
			.help("Don't show any output"))
		.arg(Arg::with_name("secure")
			.short("s")
			.long("secure")
			.help("Enables TLS on the server")
			.requires_all(&["key_file", "cert_file"]))
		.arg(Arg::with_name("verbose")
			.short("v")
			.long("verbose")
			.help("Enables verbose logging")
			.conflicts_with("quiet"))
		.arg(Arg::with_name("key_file")
			.long("key_file")
			.help("The key file, if you are running the server with TLS")
			.takes_value(true))
		.arg(Arg::with_name("cert_file")
			.long("cert_file")
			.help("The certificate, if you are running the server with TLS")
			.takes_value(true))
		.get_matches();

	let mut conf = CONFIG.write().await;
	let parsed_correctly = conf.parse_args(matches);
	drop(conf);

	if !parsed_correctly {
		exit(1);
	}

	let registrations: Registrations = Arc::new(RwLock::new(HashMap::new()));

	let cors = warp::cors()
		.allow_method(warp::hyper::Method::GET)
		.allow_header(warp::hyper::header::CONTENT_TYPE)
		.allow_any_origin()
		.build();

	let register_route = warp::path("register")
		.and(warp::get())
		.and(warp::query())
		.and(with_registrations(registrations.clone()))
		.and_then(Registration::new_handler)
		.with(&cors);

	let connect_route = warp::path("connect")
		.and(warp::ws())
		.and(warp::query())
		.and(with_registrations(registrations.clone()))
		.and_then(Socket::connect_handler)
		.with(&cors);

	let remove_route = warp::path("remove")
		.and(warp::get())
		.and(warp::query())
		.and(with_registrations(registrations.clone()))
		.and_then(Registration::remove_handler)
		.with(cors);

	let routes = register_route
		.or(connect_route)
		.or(remove_route);

	let conf = CONFIG.read().await;
	let port = conf.port;

	let ip_addr = match std::net::UdpSocket::bind(format!("0.0.0.0:{}", port)) {
		Ok(sock) => match sock.connect("8.8.8.8:80") {
			Ok(_) => match sock.local_addr() {
				Ok(addr) => Some(addr.ip().to_string()),
				_ => None
			},
			_ => None
		},
		_ => None
	};

	let log_str = match ip_addr {
		Some(ip) => format!(" at \x1b[1m{}:{}\x1b[0m", ip, port),
		None => "".to_owned(),
	};

	if conf.secure {
		log!(!conf.quiet, Color::Blue, "Running server{} with TLS", log_str);

		let key_path = conf.key_file
			.as_ref()
			.expect("Please provide a key file")
			.to_owned();

		let cert_path = conf.cert_file
			.as_ref()
			.expect("Please provide a cert file")
			.to_owned();

		drop(conf);
		warp::serve(routes)
			.tls()
			.cert_path(cert_path)
			.key_path(key_path)
			.run(([0, 0, 0, 0], port))
			.await

	} else {
		log!(!conf.quiet, Color::Blue, "Running server{}...", log_str);

		drop(conf);
		warp::serve(routes).run(([0, 0, 0, 0], port)).await;
	}
}

fn with_registrations(
	rgs: Registrations
) -> impl Filter<Extract = (Registrations,), Error = Infallible> + Clone {
	warp::any().map(move || rgs.clone())
}

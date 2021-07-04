use uuid::Uuid;

#[macro_export]
macro_rules! log_vbs{
	($vbs:expr, $out:expr, $msg:expr$(, $args:expr)*) => {
		if $vbs {
			crate::log!($out, Color::Purple, $msg$(, $args)*)
		}
	}
}

#[macro_export]
macro_rules! log{
	($out:expr, $col:expr, $msg:expr$(, $args:expr)*) => {
		if $out {
			Config::log(format!($msg$(, $args)*), $col);
		}
	}
}

#[macro_export]
macro_rules! err{
	($out:expr, $msg:expr$(, $args:expr)*) => {
		if $out {
			Config::err(format!($msg$(, $args)*))
		}
	}
}

pub struct Config {
	pub port: u16,
	pub quiet: bool,
	pub verbose: bool,
	pub secret_key: String,
	pub secure: bool,
	pub auto_remove: bool,
	pub key_file: Option<String>,
	pub cert_file: Option<String>
}

impl Config {
	pub fn default() -> Config {
		Config {
			port: 8741,
			quiet: false,
			verbose: false,
			secret_key: Uuid::new_v4().to_string(),
			secure: false,
			auto_remove: false,
			key_file: None,
			cert_file: None
		}
	}

	pub fn parse_args(&mut self, matches: clap::ArgMatches) -> bool {
		if matches.occurrences_of("quiet") > 0 {
			self.quiet = true;
		}

		if let Some(port) = matches.value_of("port") {
			if let Ok(port_int) = port.parse() {
				self.port = port_int;
			} else {
				err!(!self.quiet, "Please only use values from 0 = 2^16 for the port (you input '{}')", port);
				return false;
			}
		}

		if let Some(key) = matches.value_of("secret_key") {
			self.secret_key = key.to_owned();
		}

		if matches.occurrences_of("secure") > 0 {
			self.key_file = matches.value_of("key_file")
				.map(|k| k.to_owned());

			self.cert_file = matches.value_of("cert_file")
				.map(|c| c.to_owned());

			if self.cert_file.is_none() || self.key_file.is_none() {
				err!(!self.quiet, "Please enter both a key_file and a cert_file");
				return false;
			}

			self.secure = true;
		}

		if matches.occurrences_of("verbose") > 0 {
			self.verbose = true;
		}

		if matches.occurrences_of("remove") > 0 {
			self.auto_remove = true;
		}

		true
	}

	pub fn err(err: String) {
		eprintln!("\x1b[1m{} \x1b[31m✗\x1b[0m  {}", chrono::Local::now().format("[%H:%M:%S]"), err);
	}

	pub fn log(log: String, color: Color) {
		let col_str = match color {
			Color::Green => "32",
			Color::Yellow => "33",
			Color::Blue => "34",
			Color::Purple => "35",
		};

		println!("\x1b[1m{} \x1b[{}m=>\x1b[0m {}", chrono::Local::now().format("[%H:%M:%S]"), col_str, log);
	}

	pub async fn out_and_vbs() -> (bool, bool) {
		let conf = crate::CONFIG.read().await;
		(!conf.quiet, conf.verbose)
	}
}

pub enum Color {
	Green,
	Yellow,
	Blue,
	Purple,
}

use uuid::Uuid;

pub struct Config {
	pub port: u16,
	pub quiet: bool,
	pub secret_key: String,
	pub secure: bool,
	pub key_file: Option<String>,
	pub cert_file: Option<String>
}

impl Config {
	pub fn default() -> Config {
		Config {
			port: 8741,
			quiet: false,
			secret_key: Uuid::new_v4().to_string(),
			secure: false,
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
				Config::err(&format!("Please only use values from 0 - 2^16 for the post (you input '{}')", port), !self.quiet);
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
				Config::err("Please enter both a key_file and a cert_file", !self.quiet);
				return false;
			}
		}

		true
	}

	pub fn err(err: &str, print: bool) {
		if print {
			eprintln!("\x1b[31;1m✗\x1b[0m {}", err);
		}
	}

	pub fn log(log: &str, print: bool, color: Color) {
		if print {
			let col_str = match color {
				Color::Green => "32",
				Color::Yellow => "33",
				Color::Blue => "34",
			};

			println!("\x1b[{};1m=>\x1b[0m {}", col_str, log);
		}
	}
}

pub enum Color {
	Green,
	Yellow,
	Blue,
}

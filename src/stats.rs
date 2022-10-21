use crate::{
	config::{Color, Config},
	log, Registrations,
};
use std::convert::TryInto;
use sysinfo::{ProcessExt, SystemExt};
use warp::{Rejection, Reply};

pub async fn return_stats(rgs: Registrations) -> Result<impl Reply, Rejection> {
	log!(true, Color::Yellow, "Requesting stats on server...");

	let regs = rgs.read().await;

	let mut reg_info = Vec::new();

	for (k, r) in regs.iter() {
		let conns = r.connections.read().await;
		let con_len = conns.len();
		drop(conns);

		let destroy = *(r.destroy.read().await);

		reg_info.push(serde_json::json!({
			"id": k,
			"connections": con_len,
			"reg_type": format!("{:?}", r.reg_type),
			"destroy": destroy
		}));
	}

	let mut system = sysinfo::System::new_all();
	system.refresh_memory();

	let proc_usage = std::process::id()
		.try_into()
		.ok()
		.and_then(|pid| {
			system.refresh_process(pid);

			system.process(pid)
				.map(|proc| proc.memory() as i64)
		})
		.unwrap_or(-1);

	let sys_info = serde_json::json!({
		"total_mem": system.total_memory(),
		"used_mem": system.used_memory(),
		"proc_mem": proc_usage
	});

	let ret = serde_json::json!({
		"registrations": reg_info,
		"system": sys_info
	});

	Ok(ret.to_string())
}

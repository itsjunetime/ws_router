use crate::{
	Registrations,
	log,
	config::{Config, Color}
};
use warp::{Reply, Rejection};
use sysinfo::{SystemExt, ProcessExt};
use std::convert::TryInto;

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

	let mut proc_usage = -1;

	if let Ok(pid) = std::process::id().try_into() {
		system.refresh_process(pid);

		if let Some(proc) = system.process(pid) {
			proc_usage = proc.memory() as i64;
		}
	}

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

use crate::{appconfig::AppConfig, watcher};
use chrono::NaiveDateTime;
use notify::PollWatcher;
use std::{
	error::Error,
	fmt::{self, Display},
	time::Duration,
};
use tokio::{
	fs::File,
	io::{AsyncBufReadExt, AsyncSeekExt, BufReader, Lines},
	sync::mpsc::{unbounded_channel, UnboundedReceiver},
};
use tokio_retry::{strategy::FixedInterval, Retry};

#[derive(Debug, Clone)]
pub struct LastResetData {
	pub num: i64,
	pub timestamp_utc_ms: i64,
}

impl Default for LastResetData {
	fn default() -> Self {
		Self {
			num: 0,
			timestamp_utc_ms: chrono::offset::Utc::now().timestamp_millis(),
		}
	}
}

impl LastResetData {
	fn parse_line(&mut self, line: &str) -> Option<LastResetData> {
		if line.contains("[INFO] Running version") {
			*self = LastResetData::default();
			self.parse_time(line);
			return Some(self.clone());
		}
		if line.contains("Reset: full")
			|| line.contains("Reset: yaw")
			|| line.contains("Reset: quick")
			|| line.contains("Reset: fast")
		{
			self.num += 1;
			self.parse_time(line);
			return Some(self.clone());
		}
		None
	}

	fn parse_time(&mut self, line: &str) {
		let timestamp_str = line.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
		if let Ok(timestamp) = NaiveDateTime::parse_from_str(timestamp_str.as_str(), "%Y-%m-%d %H:%M:%S") {
			self.timestamp_utc_ms = timestamp
				.and_local_timezone(chrono::Local::now().timezone())
				.unwrap()
				.timestamp_millis();
		}
	}
}

#[derive(Debug)]
struct WatcherCreateError;
impl Display for WatcherCreateError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "Failed to create watcher")
	}
}
impl Error for WatcherCreateError {}

async fn is_no_new_data(lines: &mut Lines<BufReader<File>>) -> bool {
	let file = &mut lines.get_mut();
	if let Ok(pos) = file.stream_position().await {
		let file = &lines.get_ref().get_ref();
		if let Ok(meta) = file.metadata().await {
			let len = meta.len();
			if pos == len {
				return true;
			}
		}
	}
	false
}

pub fn watch_resets() -> UnboundedReceiver<LastResetData> {
	let (tx, rx) = unbounded_channel();
	let filename = AppConfig::get_log_path_string();

	tokio::spawn(async move {
		let mut last_reset = LastResetData::default();

		loop {
			let (mut lines, _watcher, mut rx_notify) = Retry::spawn(FixedInterval::from_millis(1000), || async {
				let file = match File::open(&filename).await {
					Ok(file) => file,
					Err(_) => return Err(WatcherCreateError),
				};
				let (watcher, rx_notify) =
					match watcher::create_watcher::<PollWatcher>(&filename, Duration::from_millis(100)) {
						Ok((watcher, rx_notify)) => (watcher, rx_notify),
						Err(_) => return Err(WatcherCreateError),
					};
				let lines = BufReader::new(file).lines();
				Ok((lines, watcher, rx_notify))
			})
			.await
			.unwrap();

			loop {
				tokio::select! {
					Ok(Some(line)) = lines.next_line() => {
						if let Some(event) = last_reset.parse_line(line.as_str()) {
							tx.send(event).unwrap();
						}
					}
					Some(event) = rx_notify.recv() => {
						match event {
							Ok(notify::Event { kind: notify::event::EventKind::Modify(ev), .. }) => {
								match ev {
									notify::event::ModifyKind::Data(_) => {
										// got an event but no new lines -> server restarted, reopen
										if is_no_new_data(&mut lines).await {
											break;
										}
									},
									_ => { },
								}
							},
							Err(_) => { break; }
							_ => {}
						}
					}
				}
			}
		}
	});

	rx
}

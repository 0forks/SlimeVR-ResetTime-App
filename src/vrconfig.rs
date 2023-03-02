use config::ConfigError;
use notify::RecommendedWatcher;
use std::sync::RwLock;
use std::time::Duration;
use tokio::sync::mpsc::{channel, unbounded_channel, UnboundedReceiver};
use tokio_retry::strategy::FixedInterval;
use tokio_retry::Retry;

use crate::appconfig::AppConfig;
use crate::watcher;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VRConfigSkeletonToggles {
	pub skating_correction: Option<bool>,
	pub floor_clip: Option<bool>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VRConfigSkeleton {
	pub toggles: Option<VRConfigSkeletonToggles>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VRConfigLegTweaks {
	pub correction_strength: Option<f32>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VRConfig {
	pub skeleton: Option<VRConfigSkeleton>,
	pub leg_tweaks: Option<VRConfigLegTweaks>,
}

impl VRConfig {
	pub fn new() -> Result<Self, ConfigError> {
		let config = config::Config::builder()
			.add_source(config::File::with_name(&AppConfig::get_vrconfig_path_string()).required(false))
			.build()?;

		config.try_deserialize::<VRConfig>()
	}

	pub fn reload() {
		if let Ok(new_config) = VRConfig::new() {
			let mut vrconfig = VRCONFIG.write().unwrap();
			*vrconfig = new_config;
		}
	}
}

#[derive(Debug, Clone)]
pub enum Event {
	Reload,
}

pub fn watch_vrconfig() -> UnboundedReceiver<Event> {
	let (tx, rx) = unbounded_channel();
	let filename = AppConfig::get_vrconfig_path_string();

	tokio::spawn(async move {
		loop {
			VRConfig::reload();

			let (tx_debounced, mut rx_debounced) = channel(1);
			let mut debounce_handle = tokio::task::spawn(async {});

			let (_watcher, mut rx_notify) = Retry::spawn(FixedInterval::from_millis(1000), || async {
				watcher::create_watcher::<RecommendedWatcher>(&filename, Duration::from_millis(100))
			})
			.await
			.unwrap();

			loop {
				tokio::select! {
					Some(fs_event) = rx_notify.recv() => {
						match fs_event {
							Ok(notify::Event { kind: notify::event::EventKind::Modify(_), .. }) => {
								let tx_debounced = tx_debounced.clone();
								debounce_handle.abort();
								debounce_handle = tokio::task::spawn(async move {
									tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
									tx_debounced.send(()).await.unwrap();
								});
							},
							Err(_) => { break; },
							_ => { }
						}
					}
					Some(_) = rx_debounced.recv() => {
						VRConfig::reload();
						tx.send(Event::Reload).unwrap();
					}
				}
			}
		}
	});

	rx
}

lazy_static::lazy_static! {
	pub static ref VRCONFIG: RwLock<VRConfig> = RwLock::new(VRConfig::new().unwrap());
}

use config::{Config, ConfigError, File};
use std::fs;
use std::path::Path;
use std::sync::RwLock;

static APPCONFIG_FILE: &str = "appconfig.toml";

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct AppConfigSlimeVR {
	dir: String,
	log: String,
	vrconfig: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct AppConfigOBS {
	pub host: String,
	pub port: u16,
	pub password: String,
	pub text_time: String,
	pub text_time_format: String,
	pub text_config: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct AppConfig {
	slimevr: AppConfigSlimeVR,
	pub obs: AppConfigOBS,
}

impl AppConfig {
	pub fn new() -> Result<Self, ConfigError> {
		AppConfig::load()
	}

	fn load() -> Result<Self, ConfigError> {
		let config = Config::builder()
			.add_source(File::with_name(APPCONFIG_FILE).required(false))
			.set_default("slimevr.dir", "C:\\Program Files (x86)\\SlimeVR Server\\")?
			.set_default("slimevr.log", "log_last_0.log")?
			.set_default("slimevr.vrconfig", "vrconfig.yml")?
			.set_default("obs.host", "localhost")?
			.set_default("obs.port", 4455)?
			.set_default("obs.password", "")?
			.set_default("obs.text_time", "slimetime")?
			.set_default("obs.text_time_format", "Reset #{num} {time}")?
			.set_default("obs.text_config", "slimeconfig")?
			.build()?;

		config.try_deserialize::<AppConfig>()
	}

	pub fn make_if_missing() {
		if Path::new(APPCONFIG_FILE).exists() {
			return;
		}
		let config = APPCONFIG.read().unwrap();
		let toml_string = toml::to_string(&*config).expect("AppConfig: serialization failed");
		fs::write(APPCONFIG_FILE, toml_string).expect("AppConfig: write failed");
	}

	pub fn get_log_path_string() -> String {
		let slimevr = &APPCONFIG.read().unwrap().slimevr;
		Path::new(&slimevr.dir).join(&slimevr.log).display().to_string()
	}

	pub fn get_vrconfig_path_string() -> String {
		let slimevr = &APPCONFIG.read().unwrap().slimevr;
		Path::new(&slimevr.dir).join(&slimevr.vrconfig).display().to_string()
	}
}

lazy_static::lazy_static! {
	pub static ref APPCONFIG: RwLock<AppConfig> = RwLock::new(AppConfig::new().unwrap());
}

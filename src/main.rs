mod appconfig;
mod obs;
mod reset_tracker;
mod vrconfig;
mod watcher;

use appconfig::{AppConfig, APPCONFIG};
use chrono::Timelike;
use console::{style, Term};
use log::{info, LevelFilter};
use log4rs::{append::file::FileAppender, encode::pattern::PatternEncoder};
use obs::SetTextRequest;
use reset_tracker::LastResetData;
use std::{collections::HashMap, time::Duration};
use tokio::{sync::mpsc::Sender, time::interval};

#[tokio::main]
pub async fn main() {
	AppConfig::make_if_missing();
	init_log().unwrap();
	App::new().run().await;
}

fn init_log() -> Result<(), Box<dyn std::error::Error>> {
	let logfile = FileAppender::builder()
		.encoder(Box::new(PatternEncoder::new("{d(%Y-%m-%d %H:%M:%S)} [{l}] {m}\n")))
		.build("resettime.log")?;
	let config = log4rs::Config::builder()
		.appender(log4rs::config::Appender::builder().build("logfile", Box::new(logfile)))
		.build(
			log4rs::config::Root::builder()
				.appender("logfile")
				.build(LevelFilter::Info),
		)?;
	log4rs::init_config(config)?;
	info!("Started");
	Ok(())
}

struct RenderedStatus {
	reset: String,
	floor_clip: String,
	floor_clip_status: String,
	floor_clip_on: bool,
	skating_correction: String,
	skating_correction_status: String,
	skating_correction_on: bool,
}

struct App {
	term: Term,
	last_reset: LastResetData,
	obs_ok: bool,
	obs_error: Option<String>,
}

impl App {
	fn new() -> Self {
		Self {
			term: Term::buffered_stdout(),
			last_reset: LastResetData::default(),
			obs_ok: false,
			obs_error: None,
		}
	}

	async fn run(&mut self) {
		self.term.set_title("SlimeVR Reset Time");
		self.term.clear_screen().unwrap_or_default();
		self.term.hide_cursor().unwrap_or_default();

		let (obs_tx_ev, mut obs_rx_ev) = tokio::sync::mpsc::channel::<obs::Event>(1);
		let (obs_tx_req, obs_rx_req) = tokio::sync::mpsc::channel::<obs::SetTextRequest>(2);
		obs::run(obs_tx_ev, obs_rx_req);

		let mut tick = interval(Duration::from_secs(1));
		let mut config_watcher = vrconfig::watch_vrconfig();
		let mut reset_tracker = reset_tracker::watch_resets();

		loop {
			tokio::select! {
				_ = tick.tick() => { },
				e = obs_rx_ev.recv() => if let Some(e) = e {
					match e {
						obs::Event::Connected => {
							self.obs_ok = true;
							self.obs_error = None;
						},
						obs::Event::Disconnected => { self.obs_ok = false; },
						obs::Event::Error(err) => { self.obs_error = Some(err); },
					}
				},
				Some(data) = reset_tracker.recv() => {
					let old_status = self.render_status();
					let next_time_since_reset_ms =
						chrono::offset::Utc::now().timestamp_millis() - data.timestamp_utc_ms;
					if self.last_reset.num > 0 &&
						data.num > self.last_reset.num &&
						next_time_since_reset_ms < 2000 {
						info!("{}", old_status.reset);
					}
					self.last_reset = data;
				},
				_ = config_watcher.recv() => { },
			};

			self.render(&obs_tx_req);
		}
	}

	fn render(&self, obs_tx_req: &Sender<obs::SetTextRequest>) {
		let status = self.render_status();
		self.render_cli(&status).unwrap();
		self.render_obs(&status, &obs_tx_req);
	}

	fn render_status(&self) -> RenderedStatus {
		let time_since_reset_ms = chrono::offset::Utc::now().timestamp_millis() - self.last_reset.timestamp_utc_ms;
		let time_since_reset = chrono::NaiveDateTime::from_timestamp_millis(time_since_reset_ms).unwrap();

		let mut floor_clip_on = false;
		let mut skating_correction_on = false;
		let mut correction_strength: f32 = 0.0;

		let config = vrconfig::VRCONFIG.read().unwrap();
		if let Some(skeleton) = config.skeleton.as_ref() {
			if let Some(toggles) = skeleton.toggles.as_ref() {
				floor_clip_on = toggles.floor_clip.unwrap_or(false);
				skating_correction_on = toggles.skating_correction.unwrap_or(false);
			}
		}
		if let Some(leg_tweaks) = config.leg_tweaks.as_ref() {
			correction_strength = leg_tweaks.correction_strength.unwrap_or(0.0);
		}

		let reset_format = APPCONFIG.read().unwrap().obs.text_time_format.to_owned();
		let mut reset_vars = HashMap::new();
		reset_vars.insert("num".to_owned(), self.last_reset.num.to_string());
		reset_vars.insert(
			"time".to_owned(),
			if time_since_reset.hour() > 0 {
				time_since_reset.format("%H:%M:%S")
			} else {
				time_since_reset.format("%M:%S")
			}
			.to_string(),
		);

		RenderedStatus {
			reset: strfmt::strfmt(&reset_format, &reset_vars)
				.unwrap_or_else(|e| format!("Invalid reset time format: {}", e.to_string())),
			floor_clip: "Floor clip: ".to_string(),
			floor_clip_on,
			floor_clip_status: if floor_clip_on { "ON" } else { "OFF" }.to_string(),
			skating_correction: "Skating correction: ".to_string(),
			skating_correction_on,
			skating_correction_status: format!(
				"{}{}",
				if skating_correction_on { "ON" } else { "OFF" },
				if correction_strength > 0.0 {
					format!(", {:.0}%", correction_strength * 100.0)
				} else {
					"".to_owned()
				}
			),
		}
	}

	fn render_cli(&self, s: &RenderedStatus) -> std::io::Result<()> {
		let term = &self.term;
		term.move_cursor_to(0, 0)?;
		term.clear_line()?;
		term.write_line(s.reset.as_str())?;
		term.write_line("")?;

		let fc_status = style(s.floor_clip_status.to_owned());
		let fc_status = if s.floor_clip_on {
			fc_status.cyan()
		} else {
			fc_status.magenta()
		};

		term.clear_line()?;
		term.write_line(&format!("{}{}", s.floor_clip, fc_status))?;

		let sc_status = style(s.skating_correction_status.to_owned());
		let sc_status = if s.skating_correction_on {
			sc_status.cyan()
		} else {
			sc_status.magenta()
		};

		term.clear_line()?;
		term.write_line(&format!("{}{}", s.skating_correction, sc_status))?;
		term.write_line("")?;

		term.clear_line()?;
		if self.obs_ok {
			term.write_line(&format!("OBS: {}", style("OK").cyan()))?;
		} else if let Some(obs_error) = self.obs_error.as_ref() {
			term.write_line(&format!("OBS: {}", style(obs_error).magenta()))?
		} else {
			term.write_line(&format!("OBS: {}", style("Disconnected").magenta()))?;
		}
		term.clear_to_end_of_screen()?;
		term.flush()?;

		Ok(())
	}

	fn render_obs(&self, s: &RenderedStatus, obs_tx_req: &Sender<obs::SetTextRequest>) {
		let obs = &APPCONFIG.read().unwrap().obs;
		obs_tx_req
			.try_send(SetTextRequest {
				element: obs.text_time.to_owned(),
				text: s.reset.to_owned(),
			})
			.unwrap_or_default();
		obs_tx_req
			.try_send(SetTextRequest {
				element: obs.text_config.to_owned(),
				text: vec![
					vec![s.floor_clip.to_owned(), s.floor_clip_status.to_owned()].join(""),
					vec![s.skating_correction.to_owned(), s.skating_correction_status.to_owned()].join(""),
				]
				.join("\n"),
			})
			.unwrap_or_default();
	}
}

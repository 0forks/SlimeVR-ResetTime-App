use crate::appconfig::APPCONFIG;
use tokio::sync::mpsc::{Sender, Receiver};

#[derive(serde::Serialize)]
struct SetTextSettings {
	text: String,
}

#[derive(Debug, Clone)]
pub struct SetTextRequest {
	pub element: String,
	pub text: String,
}

#[derive(Debug, Clone)]
pub enum Event {
	Disconnected,
	Connected,
	Error(String),
}

pub fn run(tx_ev: Sender<Event>, mut rx_req: Receiver<SetTextRequest>) {
	tokio::spawn(async move {
		loop {
			let config = APPCONFIG.read().unwrap().obs.clone();
			match obws::Client::connect(config.host, config.port, Some(config.password)).await {
				Ok(client) => {
					tx_ev.send(Event::Connected).await.unwrap();
					process(client, &tx_ev, &mut rx_req).await;
				}
				Err(err) => {
					tx_ev.send(Event::Disconnected).await.unwrap();
					tx_ev.send(Event::Error(format!("{}", err))).await.unwrap();
					tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
					continue;
				}
			}
		}
	});
}

async fn process(client: obws::Client, tx_ev: &Sender<Event>, rx_req: &mut Receiver<SetTextRequest>) {
	while let Some(request) = rx_req.recv().await {
		let result = client.inputs().set_settings(obws::requests::inputs::SetSettings {
			input: request.element.as_str(),
			settings: &SetTextSettings {
				text: request.text
			},
			overlay: None,
		}).await;

		if let Err(err) = result {
			match err {
				obws::Error::Api { code: _, message: _ } => { 
					// field does not exist
				},
				_ => {
					tx_ev.send(Event::Disconnected).await.unwrap();
					tx_ev.send(Event::Error(format!("{}", err))).await.unwrap();
					break;
				},
			}
		}
	}
}

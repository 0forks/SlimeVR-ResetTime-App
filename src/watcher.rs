use notify::{Error, Event, RecursiveMode, Watcher};
use std::{path::Path, time::Duration};
use tokio::sync::mpsc::UnboundedReceiver;

pub fn create_watcher<T: Watcher>(
	filename: &String,
	poll_interval: Duration,
) -> notify::Result<(T, UnboundedReceiver<Result<Event, Error>>)> {
	let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
	let mut watcher = T::new(
		move |res| {
			futures::executor::block_on(async {
				tx.send(res).unwrap();
			})
		},
		notify::Config::default()
			.with_poll_interval(poll_interval)
			.with_compare_contents(true),
	)?;
	watcher.watch(Path::new(&filename), RecursiveMode::NonRecursive)?;

	Ok((watcher, rx))
}

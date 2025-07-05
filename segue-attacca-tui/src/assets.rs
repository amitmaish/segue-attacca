use strum::Display;
use tokio::sync::oneshot;

#[derive(Debug, Default, Display)]
pub enum Asset<T> {
    Some(T),
    Loading(oneshot::Receiver<Asset<T>>),
    #[default]
    Unloaded,
    LoadError(Box<str>),
    None,
}

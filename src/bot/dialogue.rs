use serde::{Deserialize, Serialize};
pub use teloxide::dispatching::dialogue::InMemStorage;

#[derive(Clone, Default, Serialize, Deserialize)]
pub enum State {
    #[default]
    Start,
    ReceiveRepoUrl,
}

pub type Dialogue = teloxide::dispatching::dialogue::Dialogue<State, InMemStorage<State>>;

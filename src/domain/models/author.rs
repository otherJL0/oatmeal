use serde::Deserialize;
use serde::Serialize;

use crate::configuration::Config;
use crate::configuration::ConfigKey;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Author {
    User,
    Oatmeal,
    Model,
}

impl std::fmt::Display for Author {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let output = match self {
            Author::User => Config::get(ConfigKey::Username),
            Author::Oatmeal => String::from("Oatmeal"),
            Author::Model => Config::get(ConfigKey::Model),
        };
        return write!(f, "{output}");
    }
}

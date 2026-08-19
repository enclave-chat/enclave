use std::borrow::Cow;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientMeta {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerMeta {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method")]
pub enum ClientMethod {
    Initialized {
        public_key: String,
        signature: String,

        timestamp: u64,
        hostname: String,
    },

    Error {
        error: Cow<'static, str>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method")]
pub enum ServerMethod {
    Initialize {
        public_key: String,
        signature: String,

        timestamp: u64,
        hostname: String,
    },

    Meta(ClientMeta),

    Error {
        error: String,
    },
}

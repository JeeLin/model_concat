pub mod http;
pub mod websocket;

pub mod api_provider;
pub mod registry;

use self::{http::HttpConfig, websocket::WsConfig};

#[derive(Debug, Clone, Default)]
pub struct AdapterConfig {
    pub http: HttpConfig,
    pub ws: WsConfig,
}

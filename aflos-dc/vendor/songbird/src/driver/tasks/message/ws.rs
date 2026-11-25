#![allow(missing_docs)]

use super::Interconnect;
use crate::{model::Event as GatewayEvent, ws::WsStream};

pub enum WsMessage {
    Ws(Box<WsStream>),
    ReplaceInterconnect(Interconnect),
    SetKeepalive(f64),
    Speaking(bool),
    Deliver(GatewayEvent),
    /// aflos-dc: a message the model doesn't have (op 12 with our streams).
    Json(serde_json::Value),
}

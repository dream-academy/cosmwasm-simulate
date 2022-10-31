use cosmwasm_std::{Attribute, Binary, Event, Response};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, Eq)]
pub struct DebugLog {
    pub logs: Vec<DebugLogEntry>,
    pub err_msg: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, Eq)]
pub struct DebugLogEntry {
    pub attributes: Vec<Attribute>,
    pub events: Vec<Event>,
    pub data: Option<Binary>,
}

impl DebugLog {
    pub fn new() -> Self {
        Self {
            logs: Vec::new(),
            err_msg: None,
        }
    }

    pub fn set_err_msg(&mut self, err_msg: &str) {
        self.err_msg = Some(err_msg.to_string());
    }

    pub fn append(&mut self, response: &Response) {
        self.logs.push(DebugLogEntry {
            attributes: response.attributes.clone(),
            events: response.events.clone(),
            data: response.data.clone(),
        });
    }
}

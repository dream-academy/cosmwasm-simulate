use cosmwasm_std::{Attribute, Binary, Event, Response};
use serde::{Deserialize, Serialize};
use serde_json::to_string;
use std::collections::HashMap;
use std::fmt;

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, Eq)]
pub struct DebugLog {
    pub logs: Vec<DebugLogEntry>,
    pub err_msg: Option<String>,
    pub stdout: Vec<String>,
    pub code_coverage: HashMap<String, Vec<Vec<u8>>>,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, Eq)]
pub struct DebugLogEntry {
    pub attributes: Vec<Attribute>,
    pub events: Vec<Event>,
    pub data: Option<Binary>,
}

impl fmt::Display for DebugLogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", to_string(&self).unwrap())?;
        Ok(())
    }
}

impl DebugLog {
    pub fn new() -> Self {
        Self {
            logs: Vec::new(),
            err_msg: None,
            stdout: Vec::new(),
            code_coverage: HashMap::new(),
        }
    }

    pub fn set_err_msg(&mut self, err_msg: &str) {
        self.err_msg = Some(err_msg.to_string());
    }

    pub fn append_log(&mut self, response: &Response) {
        self.logs.push(DebugLogEntry {
            attributes: response.attributes.clone(),
            events: response.events.clone(),
            data: response.data.clone(),
        });
    }

    pub fn append_stdout(&mut self, msg: &str) {
        self.stdout.push(msg.to_string())
    }

    pub fn get_stdout(&self) -> String {
        let mut rv = String::new();
        for msg in self.stdout.iter() {
            rv += msg;
        }
        rv
    }
}

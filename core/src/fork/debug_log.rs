use cosmwasm_std::{Addr, Attribute, Binary, Event, Response};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Debug)]
pub struct CallTrace {
    pub call_graph: HashMap<usize, Vec<usize>>,
    pub call_graph_labels: HashMap<usize, String>,
    call_id_counter: usize,
    current_call_id: usize,
}

impl CallTrace {
    pub fn new() -> Self {
        let mut call_graph_labels = HashMap::new();
        call_graph_labels.insert(0, "top".to_string());
        Self {
            call_graph: HashMap::new(),
            call_graph_labels,
            call_id_counter: 0,
            current_call_id: 0,
        }
    }

    /// returns the parent call_id so that it can be used for restoring current_call_id
    pub fn begin_call(&mut self, context_name: &str) -> usize {
        // increment call_id counter
        self.call_id_counter += 1;
        let call_id = self.call_id_counter;
        let parent_call_id = self.current_call_id;

        // add call_id to CFG
        self.call_graph
            .entry(parent_call_id)
            .or_insert_with(Vec::new)
            .push(call_id);
        // change current to new call_id
        self.current_call_id = call_id;
        // save name for new call_id
        self.call_graph_labels
            .insert(call_id, context_name.to_string());
        parent_call_id
    }

    /// restore to parent_call_id
    pub fn end_call(&mut self, parent_call_id: usize) {
        self.current_call_id = parent_call_id;
    }

    /// when error is called during instantiate/execute/reply
    pub fn error<T: ToString>(&mut self, error_str: T) {
        self.call_id_counter += 1;
        let call_id = self.call_id_counter;
        let parent_call_id = self.current_call_id;
        // add call_id to CFG
        self.call_graph
            .entry(parent_call_id)
            .or_insert_with(Vec::new)
            .push(call_id);
        // save name for new call_id
        self.call_graph_labels
            .insert(call_id, error_str.to_string());
    }
}

#[derive(Clone, Debug)]
pub struct DebugLog {
    pub logs: Vec<DebugLogEntry>,
    pub err_msg: Option<String>,
    pub stdout: Vec<String>,
    pub call_trace: CallTrace,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DebugLogEntry {
    pub attributes: Vec<Attribute>,
    pub events: Vec<Event>,
    pub data: Option<Binary>,
}

impl fmt::Display for DebugLogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let stringified = serde_json::to_string(&self).map_err(|_e| fmt::Error)?;
        write!(f, "{}", stringified)
    }
}

impl DebugLog {
    pub fn new() -> Self {
        Self {
            logs: Vec::new(),
            err_msg: None,
            stdout: Vec::new(),
            call_trace: CallTrace::new(),
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

    pub fn begin_instantiate(&mut self, contract_addr: &Addr, msg: &[u8]) -> usize {
        let msg_json: serde_json::Value = serde_json::from_slice(msg).unwrap();
        let context_name = format!("{}:instantiate({})", contract_addr, msg_json);
        self.call_trace.begin_call(&context_name)
    }

    pub fn end_instantiate(&mut self, parent_call_id: usize) {
        self.call_trace.end_call(parent_call_id);
    }

    pub fn begin_execute(&mut self, contract_addr: &Addr, msg: &[u8]) -> usize {
        let msg_json: serde_json::Value = serde_json::from_slice(msg).unwrap();
        let context_name = format!("{}:execute({})", contract_addr, msg_json);
        self.call_trace.begin_call(&context_name)
    }

    pub fn end_execute(&mut self, parent_call_id: usize) {
        self.call_trace.end_call(parent_call_id);
    }

    pub fn begin_reply(&mut self, contract_addr: &Addr, msg: &[u8]) -> usize {
        let msg_json: serde_json::Value = serde_json::from_slice(msg).unwrap();
        let context_name = format!("{}:reply({})", contract_addr, msg_json);
        self.call_trace.begin_call(&context_name)
    }

    pub fn end_reply(&mut self, parent_call_id: usize) {
        self.call_trace.end_call(parent_call_id);
    }

    pub fn begin_query(&mut self, contract_addr: &Addr, msg: &[u8]) -> usize {
        let msg_json: serde_json::Value = serde_json::from_slice(msg).unwrap();
        let context_name = format!("{}:query({})", contract_addr, msg_json);
        self.call_trace.begin_call(&context_name)
    }

    pub fn end_query(&mut self, parent_call_id: usize) {
        self.call_trace.end_call(parent_call_id);
    }

    pub fn begin_error<T: ToString>(&mut self, error_str: T) {
        self.call_trace.error(error_str);
    }

    pub fn get_call_trace(&self) -> (HashMap<usize, Vec<usize>>, HashMap<usize, String>) {
        (
            self.call_trace.call_graph.clone(),
            self.call_trace.call_graph_labels.clone(),
        )
    }
}

use crate::contract_vm::Error;

pub struct Bank {}

impl Bank {
    pub fn new() -> Result<Self, Error> {
        Ok(Bank {})
    }
}

use std::collections::BTreeMap;
use std::fmt;
#[cfg(feature = "iterator")]
use std::iter;

use crate::contract_vm::watcher;
use cosmwasm_std::{Coin, Order};
use cosmwasm_vm::{Backend, BackendApi, BackendError, BackendResult, GasInfo, Storage};

use cosmwasm_std::Record;

///mock storage
#[derive(Default)]
pub struct MockStorage {
    data: BTreeMap<Vec<u8>, Vec<u8>>,
    #[cfg(feature = "iterator")]
    iterators: BTreeMap<u32, (Vec<Record>, usize)>,
    #[cfg(feature = "iterator")]
    iterator_id_ctr: u32,
}

impl fmt::Debug for MockStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MockStorage")?;
        Ok(())
    }
}

impl MockStorage {
    pub fn new() -> Self {
        MockStorage::default()
    }

    #[cfg(feature = "iterator")]
    pub fn new_iterator(&mut self, records: Vec<Record>) -> u32 {
        self.iterator_id_ctr += 1;
        self.iterators.insert(self.iterator_id_ctr - 1, (records, 0));
        self.iterator_id_ctr - 1
    }
}

impl Storage for MockStorage {
    fn get(&self, key: &[u8]) -> BackendResult<Option<Vec<u8>>> {
        (Ok(self.data.get(key).cloned()), GasInfo::free())
    }

    #[cfg(feature = "iterator")]
    fn scan(
        &mut self,
        start: Option<&[u8]>,
        end: Option<&[u8]>,
        order: Order,
    ) -> BackendResult<u32> {
        // BTreeMap.range panics if range is start > end.
        // However, this cases represent just empty range and we treat it as such.

        let range = match (start, end) {
            (Some(s), Some(e)) => {
                if start > end {
                    return (
                        Ok(self.new_iterator(vec![])),
                        GasInfo::free(),
                    );
                } else {
                    self.data.range(s.to_vec()..e.to_vec())
                }
            }
            (Some(s), None) => self.data.range(s.to_vec()..),
            (None, Some(e)) => self.data.range(..e.to_vec()),
            (None, None) => self.data.range(vec![]..),
        };
        let mut records: Vec<Record> = range.map(|(x, y)| (x.clone(), y.clone())).collect();
        match order {
            Order::Ascending => {
                (Ok(self.new_iterator(records)), GasInfo::free())
            }
            Order::Descending => {
                records.reverse();
                (Ok(self.new_iterator(records)), GasInfo::free())
            }
        }
    }

    #[cfg(feature = "iterator")]
    fn next(&mut self, iterator_id: u32) -> BackendResult<Option<Record>> {
        if let Some((records, index)) = self.iterators.get_mut(&iterator_id) {
            if *index >= records.len() {
                (Ok(None), GasInfo::free())
            }
            else {
                *index += 1;
                (Ok(Some(records[*index - 1].clone())), GasInfo::free())
            }
        } else {
            (
                Err(BackendError::IteratorDoesNotExist { id: iterator_id }),
                GasInfo::free(),
            )
        }
    }

    fn set(&mut self, key: &[u8], value: &[u8]) -> BackendResult<()> {
        self.data.insert(key.to_vec(), value.to_vec());
        watcher::logger_storage_event_insert(key, value);
        (Ok(()), GasInfo::free())
    }

    fn remove(&mut self, key: &[u8]) -> BackendResult<()> {
        self.data.remove(key);
        (Ok(()), GasInfo::free())
    }
}

impl MockStorage {}

//mock api
#[derive(Copy, Clone)]
pub struct MockApi {
    canonical_length: usize,
}

impl MockApi {
    pub fn new(canonical_length: usize) -> Self {
        MockApi { canonical_length }
    }
}

impl Default for MockApi {
    fn default() -> Self {
        Self::new(20)
    }
}

impl BackendApi for MockApi {
    fn canonical_address(&self, human: &str) -> BackendResult<Vec<u8>> {
        // Dummy input validation. This is more sophisticated for formats like bech32, where format and checksum are validated.
        if human.len() < 3 {
            return (
                Err(BackendError::user_err(
                    "Invalid input: human address too short",
                )),
                GasInfo::free(),
            );
        }
        if human.len() > self.canonical_length {
            return (
                Err(BackendError::user_err(
                    "Invalid input: human address too long",
                )),
                GasInfo::free(),
            );
        }

        let mut out = Vec::from(human);
        let append = self.canonical_length - out.len();
        if append > 0 {
            out.extend(vec![0u8; append]);
        }
        (Ok(out), GasInfo::free())
    }

    fn human_address(&self, canonical: &[u8]) -> BackendResult<String> {
        if canonical.len() != self.canonical_length {
            return (
                Err(BackendError::user_err(
                    "Invalid input: canonical address length not correct",
                )),
                GasInfo::free(),
            );
        }

        // remove trailing 0's (TODO: fix this - but fine for first tests)
        let trimmed: Vec<u8> = canonical.iter().cloned().filter(|&x| x != 0).collect();
        // decode UTF-8 bytes into string
        if let Ok(human) = String::from_utf8(trimmed) {
            (Ok(human), GasInfo::free())
        } else {
            (
                Err(BackendError::user_err(
                    "Invalid input: canonical address not decodable",
                )),
                GasInfo::free(),
            )
        }
    }
}

pub fn new_mock(
    canonical_length: usize,
    contract_balance: &[Coin],
    contract_addr: &str,
) -> Backend<MockApi, MockStorage, cosmwasm_vm::testing::MockQuerier> {
    Backend {
        storage: MockStorage::default(),
        api: MockApi::new(canonical_length),
        querier: cosmwasm_vm::testing::MockQuerier::new(&[(contract_addr, contract_balance)]),
    }
}

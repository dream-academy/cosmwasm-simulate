use crate::ContractStorage;
use cosmwasm_std::{Order, Record};
use cosmwasm_vm::{BackendError, BackendResult, GasInfo, Storage};

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, RwLock};

///mock storage
#[derive(Clone)]
pub struct RpcMockStorage {
    inner: Arc<RwLock<ContractStorage>>,
    #[cfg(feature = "iterator")]
    iterators: HashMap<u32, (Vec<Record>, usize)>,
    #[cfg(feature = "iterator")]
    iterator_id_ctr: u32,
}

impl fmt::Debug for RpcMockStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RpcMockStorage")?;
        Ok(())
    }
}

impl RpcMockStorage {
    pub fn new(inner: &Arc<RwLock<ContractStorage>>) -> Self {
        Self {
            inner: Arc::clone(inner),
            iterators: HashMap::new(),
            iterator_id_ctr: 0,
        }
    }

    #[cfg(feature = "iterator")]
    pub fn new_iterator(&mut self, records: Vec<Record>) -> u32 {
        self.iterator_id_ctr += 1;
        self.iterators
            .insert(self.iterator_id_ctr - 1, (records, 0));
        self.iterator_id_ctr - 1
    }
}

impl Storage for RpcMockStorage {
    fn get(&self, key: &[u8]) -> BackendResult<Option<Vec<u8>>> {
        (
            Ok(self.inner.read().unwrap().get(key).cloned()),
            GasInfo::free(),
        )
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

        let inner = self.inner.read().unwrap();
        let range = match (start, end) {
            (Some(s), Some(e)) => {
                if start > end {
                    drop(inner);
                    return (Ok(self.new_iterator(vec![])), GasInfo::free());
                } else {
                    inner.range(s.to_vec()..e.to_vec())
                }
            }
            (Some(s), None) => inner.range(s.to_vec()..),
            (None, Some(e)) => inner.range(..e.to_vec()),
            (None, None) => inner.range(vec![]..),
        };
        let mut records: Vec<Record> = range.map(|(x, y)| (x.clone(), y.clone())).collect();
        drop(inner);

        match order {
            Order::Ascending => (Ok(self.new_iterator(records)), GasInfo::free()),
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
            } else {
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
        self.inner
            .write()
            .unwrap()
            .insert(key.to_vec(), value.to_vec());
        (Ok(()), GasInfo::free())
    }

    fn remove(&mut self, key: &[u8]) -> BackendResult<()> {
        self.inner.write().unwrap().remove(key);
        (Ok(()), GasInfo::free())
    }
}

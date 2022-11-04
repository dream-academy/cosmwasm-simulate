use cw_storage_plus::Item;
use serde::{Deserialize, Serialize};

pub const NUMBER: Item<u64> = Item::new("check");

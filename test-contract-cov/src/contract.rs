#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{DepsMut, Env, Event, MessageInfo, Response, Uint128};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg};
use crate::state::NUMBER;

/*
// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw-semantics-test";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
*/

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let number = Uint128::new(0x1337);
    let _ = NUMBER.save(deps.storage, &number);
    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::ProcessData { data1, data2 } => process_data(deps, data1, data2),
    }
}

fn process_data(deps: DepsMut, data1: String, data2: Uint128) -> Result<Response, ContractError> {
    let number = NUMBER.load(deps.storage)?;
    if number == data2 {
        Ok(Response::new().add_event(Event::new("branch1").add_attribute("value", "1")))
    } else {
        if data1.as_str() == "DreamAcademy" {
            Ok(Response::new()
                .add_event(Event::new("branch2").add_attribute("value", format!("{}", "1"))))
        } else {
            Err(ContractError::Unauthorized {})
        }
    }
}

mod memory {
    use std::mem;
    use std::vec::Vec;

    #[repr(C)]
    pub struct Region {
        /// The beginning of the region expressed as bytes from the beginning of the linear memory
        pub offset: u32,
        /// The number of bytes available in this region
        pub capacity: u32,
        /// The number of bytes used in this region
        pub length: u32,
    }

    pub fn release_buffer(buffer: Vec<u8>) -> *mut Region {
        let region = build_region(&buffer);
        mem::forget(buffer);
        Box::into_raw(region)
    }

    pub fn build_region(data: &[u8]) -> Box<Region> {
        let data_ptr = data.as_ptr() as usize;
        build_region_from_components(
            u32::try_from(data_ptr).expect("pointer doesn't fit in u32"),
            u32::try_from(data.len()).expect("length doesn't fit in u32"),
            u32::try_from(data.len()).expect("length doesn't fit in u32"),
        )
    }

    fn build_region_from_components(offset: u32, capacity: u32, length: u32) -> Box<Region> {
        Box::new(Region {
            offset,
            capacity,
            length,
        })
    }
}

mod coverage {
    use super::memory::release_buffer;
    use minicov::capture_coverage;
    #[no_mangle]
    extern "C" fn dump_coverage() -> u32 {
        let coverage = capture_coverage();
        release_buffer(coverage) as u32
    }
}

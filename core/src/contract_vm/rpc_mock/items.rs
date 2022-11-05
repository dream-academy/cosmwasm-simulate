macro_rules! include_proto {
    ($x: literal) => {
        include!(concat!(env!("OUT_DIR"), "/", $x, ".rs"));
    };
}

pub mod rpc_items {
    pub mod cosmwasm {
        pub mod wasm {
            pub mod v1 {
                include_proto!("cosmwasm.wasm.v1");
            }
        }
    }
    pub mod cosmos {
        pub mod base {
            pub mod v1beta1 {
                include_proto!("cosmos.base.v1beta1");
            }
            pub mod query {
                pub mod v1beta1 {
                    include_proto!("cosmos.base.query.v1beta1");
                }
            }
        }
        pub mod bank {
            pub mod v1beta1 {
                include_proto!("cosmos.bank.v1beta1");
            }
        }
    }
}

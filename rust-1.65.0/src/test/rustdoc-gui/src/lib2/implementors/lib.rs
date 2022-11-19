pub trait Whatever {
    type Foo;

    fn method() {}
}

pub struct Struct;

impl Whatever for Struct {
    type Foo = u8;
}

mod traits {
    pub trait TraitToReexport {
        fn method() {}
    }
}

#[doc(inline)]
pub use traits::TraitToReexport;

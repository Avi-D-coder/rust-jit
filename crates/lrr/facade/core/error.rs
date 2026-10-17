// `core::error`.
use crate::fmt::{Debug, Display};
use crate::option::Option;

pub trait Error: Debug + Display {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Option::None
    }
    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }
    fn cause(&self) -> Option<&dyn Error> {
        Option::None
    }
}

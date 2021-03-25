extern crate log;

use std::error::Error;

type BoxResult<T> = Result<T,Box<dyn Error>>;



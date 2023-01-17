//! The data types and interaction logic for the calculator.

#![warn(clippy::all)]
#![warn(missing_copy_implementations, missing_docs, rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn, missing_debug_implementations)]
#![cfg_attr(not(debug_assertions), deny(clippy::todo))]

mod calculator;
pub use calculator::*;

mod stack;
pub use stack::*;

mod recipe;
pub use recipe::*;

mod util;

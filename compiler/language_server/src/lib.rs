#![feature(async_closure)]
#![feature(box_patterns)]
#![feature(let_chains)]

pub use crate::server::Server;

pub mod database;
pub mod features;
pub mod features_candy;
pub mod features_ir;
mod semantic_tokens;
pub mod server;
pub mod utils;

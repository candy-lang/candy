#![feature(
    anonymous_lifetime_in_impl_trait,
    async_closure,
    box_patterns,
    let_chains,
    strict_provenance
)]
#![allow(clippy::large_enum_variant)]

pub mod database;
pub mod debug_adapter;
pub mod features;
pub mod features_candy;
pub mod features_ir;
mod semantic_tokens;
pub mod server;
pub mod utils;

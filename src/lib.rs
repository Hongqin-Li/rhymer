#![deny(missing_docs)]
//! Backend-as-a-Service in Rust

#[macro_use]
extern crate log;

mod acl;
mod database;
mod error;
mod function;
mod object;
mod rhymer;
mod user;
mod validator;

pub use rhymer::Server;
pub use rhymer::ServerConfig;
pub use warp;

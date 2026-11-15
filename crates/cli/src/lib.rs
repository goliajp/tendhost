//! `tendhost`, the command line. A thin client of the server's api: every
//! command is one or two requests and a way to print the answer.

pub mod api;
pub mod commands;
pub mod config;
pub mod table;

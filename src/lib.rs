//! mint-faa — Minimal Issue & Needs Tracker 核心库。
//!
//! 全局、单机、SQLite 背书的 issue 系统 CLI。lib + bin 双层结构：
//! 所有业务模块在此汇总，`src/main.rs` 只做 clap 薄壳调用。

pub mod cli;
pub mod db;
pub mod error;
pub mod models;
pub mod output;
pub mod project;
pub mod state;
pub mod tag;

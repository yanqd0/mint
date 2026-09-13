//! db 层单测：迁移 / 打开与 WAL / machine 三组，共享用 `use super::*` 透传。

use super::*;

mod machine;
mod migrate;
mod wal_open;

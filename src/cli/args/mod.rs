//! clap 参数定义（按域拆分）：容器 / project / IO / sync；label 参数见 `crate::cli::label`。

mod container;
mod io;
mod project;
mod sync;

pub use container::*;
pub use io::*;
pub use project::*;
pub use sync::*;

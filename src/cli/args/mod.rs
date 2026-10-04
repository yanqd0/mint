//! clap 参数定义（按域拆分）：容器 / body 编辑 / project / IO / sync；label 参数见 `crate::cli::label`。

mod body;
mod container;
mod container_link;
mod io;
mod project;
mod sync;

pub use body::*;
pub use container::*;
pub use container_link::*;
pub use io::*;
pub use project::*;
pub use sync::*;

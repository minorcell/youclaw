//! Agent 工具模块聚合入口。
//!
//! 设计原则：
//! - 一个工具一个文件，避免 `agent.rs` 继续膨胀；
//! - 公共能力（如文件审批、路径处理）集中到 context；
//! - 只在这里统一导出，调用方不关心内部文件布局。
mod filesystem_context;
mod filesystem_list_dir;
mod filesystem_read_file;
mod filesystem_write_file;
mod memory_get;
mod memory_search;
mod memory_write;

pub use filesystem_context::FilesystemToolContext;
pub use filesystem_list_dir::{build_filesystem_list_dir_tool, FILESYSTEM_LIST_DIR_TOOL_NAME};
pub use filesystem_read_file::{build_filesystem_read_file_tool, FILESYSTEM_READ_FILE_TOOL_NAME};
pub use filesystem_write_file::{
    build_filesystem_write_file_tool, FILESYSTEM_WRITE_FILE_TOOL_NAME,
};
pub use memory_get::build_memory_get_tool;
pub use memory_search::build_memory_search_tool;
pub use memory_write::build_memory_write_tool;

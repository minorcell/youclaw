//! Agent tool module exports.

use aquaregia::tool::Tool;

mod edit_file;
mod bash;
mod filesystem_context;
mod list_directory;
mod memory_get;
mod memory_search;
mod read_files;
mod read_text_file;
mod search_files;
mod tool_runtime;
mod write_file;

pub use bash::{build_bash_exec_tool, BashToolContext, BASH_EXEC_TOOL_NAME};
pub use edit_file::{build_edit_file_tool, EDIT_FILE_TOOL_NAME};
pub use filesystem_context::FilesystemToolContext;
pub(crate) use tool_runtime::{ToolRuntimeContext, INTERNAL_TOOL_CALL_ID_FIELD};
pub use list_directory::{build_list_directory_tool, LIST_DIRECTORY_TOOL_NAME};
pub use memory_get::build_memory_get_tool;
pub use memory_search::build_memory_search_tool;
pub use read_files::{build_read_files_tool, READ_FILES_TOOL_NAME};
pub use read_text_file::{build_read_text_file_tool, READ_TEXT_FILE_TOOL_NAME};
pub use search_files::{build_search_files_tool, SEARCH_FILES_TOOL_NAME};
pub use write_file::{build_write_file_tool, WRITE_FILE_TOOL_NAME};

#[derive(Clone, Copy)]
struct FilesystemToolDefinition {
    name: &'static str,
    action: &'static str,
    builder: fn(FilesystemToolContext) -> Tool,
}

const FILESYSTEM_TOOL_DEFINITIONS: [FilesystemToolDefinition; 6] = [
    FilesystemToolDefinition {
        name: LIST_DIRECTORY_TOOL_NAME,
        action: "list_directory",
        builder: build_list_directory_tool,
    },
    FilesystemToolDefinition {
        name: READ_TEXT_FILE_TOOL_NAME,
        action: "read_text_file",
        builder: build_read_text_file_tool,
    },
    FilesystemToolDefinition {
        name: READ_FILES_TOOL_NAME,
        action: "read_files",
        builder: build_read_files_tool,
    },
    FilesystemToolDefinition {
        name: SEARCH_FILES_TOOL_NAME,
        action: "search_files",
        builder: build_search_files_tool,
    },
    FilesystemToolDefinition {
        name: WRITE_FILE_TOOL_NAME,
        action: "write_file",
        builder: build_write_file_tool,
    },
    FilesystemToolDefinition {
        name: EDIT_FILE_TOOL_NAME,
        action: "edit_file",
        builder: build_edit_file_tool,
    },
];

pub(crate) fn build_filesystem_tools(context: FilesystemToolContext) -> Vec<Tool> {
    FILESYSTEM_TOOL_DEFINITIONS
        .iter()
        .map(|definition| (definition.builder)(context.clone()))
        .collect()
}

pub(crate) fn filesystem_tool_action(tool_name: &str) -> Option<&'static str> {
    FILESYSTEM_TOOL_DEFINITIONS
        .iter()
        .find(|definition| definition.name == tool_name)
        .map(|definition| definition.action)
}

pub(crate) fn tool_action(tool_name: &str) -> Option<&'static str> {
    filesystem_tool_action(tool_name).or_else(|| {
        if tool_name == BASH_EXEC_TOOL_NAME {
            Some("exec")
        } else {
            None
        }
    })
}

pub(crate) fn is_filesystem_tool(tool_name: &str) -> bool {
    filesystem_tool_action(tool_name).is_some()
}

pub(crate) fn requires_tool_call_binding(tool_name: &str) -> bool {
    is_filesystem_tool(tool_name) || tool_name == BASH_EXEC_TOOL_NAME
}

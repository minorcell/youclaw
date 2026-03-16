//! Agent 运行期子模块。
//!
//! 当前主要承载工具系统，后续可继续扩展为 `planner` / `executor` 等模块。
pub mod context_compactor;
pub mod context_constants;
pub mod message_builder;
pub mod stream_collector;
pub mod summarizer;
pub mod token_estimator;
pub mod tool_dispatcher;
pub mod tool_result_processor;
pub mod tools;

//! 对外网关：FUSE、S3 端点、WebDAV、FTPS、SFTP、MCP Server
//!
//! 行为契约见 docs/specs/ 对应工作包规格。
//! MCP Server: RMCP 2026-07-28 stateless，工具面见 [`mcp`] 模块；
//! WASM 扩展系统接线见 [`ext`] 模块（SPEC M7-WP01 §2.3）。

pub mod ext;
pub mod mcp;

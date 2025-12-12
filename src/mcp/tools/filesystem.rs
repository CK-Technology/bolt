//! Container filesystem access tool
//!
//! Provides read/write access to container filesystem with security boundaries

use crate::mcp::{McpError, Result, tools::McpTool};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Filesystem access tool
///
/// Provides secure filesystem access within a container's root directory.
/// Prevents path traversal attacks and restricts access to configured root.
pub struct FilesystemTool {
    root: PathBuf,
}

impl FilesystemTool {
    /// Create a new filesystem tool with the given root directory
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    /// Validate and resolve a path within the container root
    fn resolve_path(&self, rel_path: &str) -> Result<PathBuf> {
        let abs_path = self.root.join(rel_path);

        // Canonicalize to prevent path traversal
        let canonical = abs_path.canonicalize().map_err(|e| McpError::Io(e))?;

        // Ensure the path is within the root
        if !canonical.starts_with(&self.root) {
            return Err(McpError::PermissionDenied(format!(
                "Path escape attempt: {} is outside container root",
                rel_path
            )));
        }

        Ok(canonical)
    }

    async fn read_file(&self, path: &str) -> Result<String> {
        let resolved = self.resolve_path(path)?;

        tracing::info!("Reading file: {}", resolved.display());

        let contents = tokio::fs::read_to_string(&resolved)
            .await
            .map_err(|e| McpError::Io(e))?;

        Ok(contents)
    }

    async fn write_file(&self, path: &str, contents: &str) -> Result<()> {
        let resolved = self.resolve_path(path)?;

        tracing::warn!(
            "Writing file: {} ({} bytes)",
            resolved.display(),
            contents.len()
        );

        tokio::fs::write(&resolved, contents)
            .await
            .map_err(|e| McpError::Io(e))?;

        Ok(())
    }

    async fn list_directory(&self, path: &str) -> Result<Vec<FileEntry>> {
        let resolved = self.resolve_path(path)?;

        tracing::info!("Listing directory: {}", resolved.display());

        let mut entries = Vec::new();
        let mut dir = tokio::fs::read_dir(&resolved)
            .await
            .map_err(|e| McpError::Io(e))?;

        while let Some(entry) = dir.next_entry().await.map_err(|e| McpError::Io(e))? {
            let metadata = entry.metadata().await.map_err(|e| McpError::Io(e))?;
            entries.push(FileEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                is_dir: metadata.is_dir(),
                size: metadata.len(),
            });
        }

        Ok(entries)
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation")]
enum FilesystemInput {
    #[serde(rename = "read")]
    Read { path: String },
    #[serde(rename = "write")]
    Write { path: String, contents: String },
    #[serde(rename = "list")]
    List { path: String },
}

#[derive(Debug, Serialize)]
struct FileEntry {
    name: String,
    is_dir: bool,
    size: u64,
}

impl McpTool for FilesystemTool {
    fn name(&self) -> &str {
        "bolt_filesystem"
    }

    fn description(&self) -> &str {
        "Access container filesystem: read files, write files, and list directories"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "oneOf": [
                {
                    "properties": {
                        "operation": { "const": "read" },
                        "path": {
                            "type": "string",
                            "description": "Path relative to container root"
                        }
                    },
                    "required": ["operation", "path"]
                },
                {
                    "properties": {
                        "operation": { "const": "write" },
                        "path": {
                            "type": "string",
                            "description": "Path relative to container root"
                        },
                        "contents": {
                            "type": "string",
                            "description": "File contents to write"
                        }
                    },
                    "required": ["operation", "path", "contents"]
                },
                {
                    "properties": {
                        "operation": { "const": "list" },
                        "path": {
                            "type": "string",
                            "description": "Directory path relative to container root"
                        }
                    },
                    "required": ["operation", "path"]
                }
            ]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let args: FilesystemInput = serde_json::from_value(input)?;

        match args {
            FilesystemInput::Read { path } => {
                let contents = self.read_file(&path).await?;
                Ok(json!({
                    "operation": "read",
                    "path": path,
                    "contents": contents,
                    "size": contents.len()
                }))
            }
            FilesystemInput::Write { path, contents } => {
                self.write_file(&path, &contents).await?;
                Ok(json!({
                    "operation": "write",
                    "path": path,
                    "bytes_written": contents.len()
                }))
            }
            FilesystemInput::List { path } => {
                let entries = self.list_directory(&path).await?;
                Ok(json!({
                    "operation": "list",
                    "path": path,
                    "entries": entries
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_path_validation() {
        let dir = tempdir().unwrap();
        let tool = FilesystemTool::new(dir.path());

        // Path traversal should fail
        let result = tool.resolve_path("../../../etc/passwd");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_write() {
        let dir = tempdir().unwrap();
        let tool = FilesystemTool::new(dir.path());

        // Write a file
        let write_result = tool.write_file("test.txt", "hello world").await;
        assert!(write_result.is_ok());

        // Read it back
        let contents = tool.read_file("test.txt").await.unwrap();
        assert_eq!(contents, "hello world");
    }
}

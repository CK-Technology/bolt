use anyhow::{Context, Result};
use libloading::{Library, Symbol};
use std::path::PathBuf;
use std::sync::Arc;

use super::{Plugin, PluginManifest};

pub struct PluginLoader {
    libraries: Vec<Arc<Library>>,
}

impl PluginLoader {
    pub fn new() -> Self {
        Self {
            libraries: Vec::new(),
        }
    }
}

pub async fn load_plugin(path: &PathBuf, manifest: &PluginManifest) -> Result<Box<dyn Plugin>> {
    let library_path = path.join(&manifest.entry_point);

    if !library_path.exists() {
        return Err(anyhow::anyhow!(
            "Plugin library not found: {:?}",
            library_path
        ));
    }

    unsafe {
        let lib = Arc::new(
            Library::new(&library_path)
                .with_context(|| format!("Failed to load plugin library: {:?}", library_path))?,
        );

        let create_plugin: Symbol<unsafe extern "C" fn() -> *mut dyn Plugin> = lib
            .get(b"create_plugin")
            .with_context(|| "Plugin must export 'create_plugin' function")?;

        let plugin_ptr = create_plugin();
        if plugin_ptr.is_null() {
            return Err(anyhow::anyhow!("Plugin creation failed"));
        }

        let plugin = Box::from_raw(plugin_ptr);
        plugin.initialize().await?;

        Ok(plugin)
    }
}

pub fn validate_plugin_signature(path: &PathBuf) -> Result<()> {
    let signature_path = path.join("plugin.sig");

    if !signature_path.exists() {
        return Err(anyhow::anyhow!("Plugin signature not found"));
    }

    Ok(())
}

#[macro_export]
macro_rules! declare_plugin {
    ($plugin_type:ty, $constructor:path) => {
        #[no_mangle]
        pub extern "C" fn create_plugin() -> *mut dyn $crate::plugins::Plugin {
            let plugin = $constructor();
            let boxed: Box<dyn $crate::plugins::Plugin> = Box::new(plugin);
            Box::into_raw(boxed)
        }
    };
}

pub use declare_plugin;

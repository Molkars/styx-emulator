// SPDX-License-Identifier: BSD-2-Clause
//! Read-only, processor-global configuration values.
//!
//! The config system provides a type-keyed store of [`ProcessorConfig`]
//! values that are set during processor construction and shared
//! read-only across subsystems at runtime. Each config type is
//! registered at most once; its concrete [`TypeId`] serves as the
//! lookup key, so different subsystems can retrieve the values they
//! care about without coupling to one another.
//!
//! See [`core_configs`] for the built-in config types supplied by the
//! processor builder.

pub mod core_configs;

use std::{any::TypeId, collections::HashMap};

use as_any::{AsAny, Downcast};

/// Marker trait for types that can be stored in a [`Config`] map.
///
/// Implementors must be `Send + Sync + 'static` so the config map can
/// be shared safely across threads. The [`AsAny`] super-trait enables
/// type-safe downcasting at retrieval time.
pub trait ProcessorConfig: Send + Sync + AsAny + 'static {}

/// Type-erased key derived from a concrete config type's [`TypeId`].
#[derive(Hash, Clone, Copy, PartialEq, Eq)]
struct ConfigId(TypeId);

impl ConfigId {
    /// Creates a key for the config type `T`.
    fn new<T: 'static>() -> Self {
        Self(TypeId::of::<T>())
    }
}

/// A type-keyed, heterogeneous map of [`ProcessorConfig`] values.
///
/// Each concrete config type may appear at most once. Values are
/// inserted during processor construction and read (but never mutated)
/// by subsystems at runtime.
///
/// Users building processors should use [`ProcessorBuilder::config()`](super::ProcessorBuilder::config) and
/// [`ProcessorBuilder::modify_config_or_default()`](super::ProcessorBuilder::modify_config_or_default) to add [`ProcessorConfig`]s
/// to their processor.
///
/// Builders of processors components that received a `&Config` can use
/// [`Config::get()`] and [`Config::get_or_default()`] to retrieve configuration
/// values.
///
/// # Examples
///
/// ```
/// use styx_processor::processor::{Config, ProcessorConfig};
///
/// #[derive(Default, Clone, Copy)]
/// struct MyConfig { stride: u64 }
/// impl ProcessorConfig for MyConfig {}
///
/// // An empty config returns `None` for unregistered types.
/// let config = Config::default();
/// assert!(config.get::<MyConfig>().is_none());
///
/// // `get_or_default` falls back to `Default::default()`.
/// assert_eq!(config.get_or_default::<MyConfig>().stride, 0);
/// ```
#[derive(Default)]
pub struct Config {
    configs: HashMap<ConfigId, Box<dyn ProcessorConfig>>,
}

impl Config {
    /// Inserts or overwrites the config value for type `C`.
    pub(crate) fn add_config<C: ProcessorConfig>(&mut self, config: C) {
        let config = Box::new(config);
        let config_id = ConfigId::new::<C>();
        self.configs.insert(config_id, config);
    }

    /// Applies `f` to the existing config of type `C`, inserting a
    /// default value first if none is present.
    pub(crate) fn modify_config_or_default<C: ProcessorConfig + Default>(
        &mut self,
        f: impl FnOnce(&mut C),
    ) {
        let config_id = ConfigId::new::<C>();
        self.configs
            .entry(config_id)
            .and_modify(move |config_item| {
                let config_downcast = config_item.as_mut().downcast_mut::<C>().expect("no");
                f(config_downcast);
            })
            .or_insert(Box::new(C::default()));
    }

    /// Returns a reference to the config of type `C`, or `None` if it
    /// has not been registered.
    ///
    /// # Examples
    ///
    /// ```
    /// use styx_processor::processor::{Config, ProcessorConfig};
    ///
    /// struct MyConfig { stride: u64 }
    /// impl ProcessorConfig for MyConfig {}
    ///
    /// let config = Config::default();
    ///
    /// // Not registered, so `get` returns `None`.
    /// assert!(config.get::<MyConfig>().is_none());
    /// ```
    pub fn get<C: ProcessorConfig>(&self) -> Option<&C> {
        let config_id = ConfigId::new::<C>();
        let config = self.configs.get(&config_id)?;
        Some(config.as_ref().downcast_ref::<C>().unwrap())
    }

    /// Returns the config of type `C`, falling back to
    /// [`Default::default`] when the entry is absent.
    ///
    /// # Examples
    ///
    /// ```
    /// use styx_processor::processor::{Config, ProcessorConfig};
    ///
    /// #[derive(Default, Clone, Copy)]
    /// struct MyConfig { stride: u64 }
    /// impl ProcessorConfig for MyConfig {}
    ///
    /// let config = Config::default();
    ///
    /// // Falls back to `MyConfig::default()` since nothing was registered.
    /// let my_config = config.get_or_default::<MyConfig>();
    /// assert_eq!(my_config.stride, 0);
    /// ```
    pub fn get_or_default<C: ProcessorConfig + Default + Copy>(&self) -> C {
        self.get().copied().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestConfig {
        a: String,
        b: u32,
    }
    impl ProcessorConfig for TestConfig {}
    #[test]
    fn test_name() {
        let mut config = Config::default();
        config.add_config(TestConfig {
            a: "hello".to_owned(),
            b: 0x1337,
        });

        let get = config.get::<TestConfig>().unwrap();
        assert_eq!(&get.a, "hello");
        assert_eq!(get.b, 0x1337);
    }
}

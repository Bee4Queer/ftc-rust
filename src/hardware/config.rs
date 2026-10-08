//! Automatically generate the hardware map configuration.

use std::{
    fmt::{Debug, Display},
    hash::Hash,
    marker::PhantomData,
    sync::atomic::AtomicBool,
};

#[cfg(feature = "proc-macro")]
pub use ftc_rust_proc::config;

use crate::hardware::Device;

/// A full config.
#[derive(Debug)]
pub struct HardwareConfig {
    pub(crate) id: &'static str,
    /// Whether it has been ensured that this config is the running one.
    pub(crate) ensured: AtomicBool,
    /// not the actual items exposed to the user, but has the proper IDs for them
    pub(crate) items: &'static [&'static str],
}

impl HardwareConfig {
    #[cfg_attr(
        not(feature = "proc-macro"),
        doc = "Create a new hardware config with the specified ID and items."
    )]
    #[cfg_attr(feature = "proc-macro", doc(hidden))]
    pub const fn new(id: &'static str, items: &'static [&'static str]) -> Self {
        Self {
            id,
            ensured: AtomicBool::new(false),
            items,
        }
    }
    /// Get the items of this config.
    pub const fn items(&self) -> &'static [&'static str] {
        self.items
    }
    /// Get the ID of this config.
    pub const fn id(&self) -> &'static str {
        self.id
    }
}

/// A hardware item usually generated with the [`config`] proc macro.
pub struct HardwareItem<T: Device> {
    pub(crate) id: &'static str,
    pub(crate) cfg: &'static HardwareConfig,
    phantom: PhantomData<T>,
}

impl<T: Device> Clone for HardwareItem<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Device> Copy for HardwareItem<T> {}

impl<T: Device> PartialEq for HardwareItem<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.cfg.id == other.cfg.id
    }
}
impl<T: Device> Eq for HardwareItem<T> {}
impl<T: Device> Hash for HardwareItem<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.cfg.id.hash(state);
    }
}

impl<T: Device> HardwareItem<T> {
    #[cfg_attr(
        not(feature = "proc-macro"),
        doc = "Create a new hardware item with the specified ID and parent config."
    )]
    #[cfg_attr(feature = "proc-macro", doc(hidden))]
    pub const fn new(id: &'static str, cfg: &'static HardwareConfig) -> Self {
        Self {
            id,
            cfg,
            phantom: PhantomData,
        }
    }
}

impl<T: Device> Debug for HardwareItem<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl<T: Device> Display for HardwareItem<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "hardware item {} @ config {}", self.id, self.cfg.id)
    }
}

//! Automatically generate the hardware map configuration.

use std::{fmt::{Debug, Display}, hash::Hash, marker::PhantomData};

use crate::hardware::Device;

#[cfg(feature = "proc-macro")]
pub use ftc_rust_proc::config;

/// A hardware item generated with the `config` proc macro.
#[repr(transparent)]
pub struct HardwareItem<T: Device>(&'static str, PhantomData<T>);

impl<T: Device> Clone for HardwareItem<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Device> Copy for HardwareItem<T> {}

impl<T: Device> PartialEq for HardwareItem<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<T: Device> Eq for HardwareItem<T> {}
impl<T: Device> Hash for HardwareItem<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<T: Device> HardwareItem<T> {
    // SAFETY: Always safe, but will be very very annoying if it's a wrong value.
    #[doc(hidden)]
    pub const unsafe fn new(id: &'static str) -> Self {
        Self(id, PhantomData)
    }
}

impl<T: Device> Debug for HardwareItem<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl<T: Device> Display for HardwareItem<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "hardware item {}", self.0)
    }
}

impl<T: Device> AsRef<str> for HardwareItem<T> {
    fn as_ref(&self) -> &str {
        self.0
    }
}
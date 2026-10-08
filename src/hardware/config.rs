//! Automatically generate the hardware map configuration.

use std::{
    fmt::{Debug, Display},
    hash::Hash,
    marker::PhantomData,
    sync::atomic::AtomicBool,
};

#[cfg(feature = "proc-macro")]
pub use ftc_rust_proc::config;
use parking_lot::RwLock;

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
    pub(crate) cached: RwLock<Option<T>>,
    phantom: PhantomData<T>,
}

impl<T: Device> Clone for HardwareItem<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            cfg: self.cfg,
            cached: RwLock::new(self.cached.read().clone()),
            phantom: PhantomData,
        }
    }
}

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
            cached: RwLock::new(None),
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

#[doc(hidden)]
#[cfg(feature = "proc-macro")]
#[allow(non_snake_case, missing_debug_implementations)]
pub mod device_docs {
    /// A DC motor.
    pub mod Motor {
        /// Generic DC motor. The PID controller for speed will not be very accurate.
        pub struct Generic;

        /// 3.7:1 reduction gearmotor.
        pub struct NeveRest37v1Gear;
        /// NeveRest classic 20 motor. Appears to no longer be produced, I can't find information on
        /// it.
        pub struct NeveRest20Gear;
        /// NeveRest classic 40 motor. 40:1 reduction gearmotor.
        pub struct NeveRest40Gear;
        /// NeveRest classic 60 motor. 60:1 reduction gearmotor.
        pub struct NeveRest60Gear;

        /// Discontinued 20:1 spur gearbox hex motor.
        pub struct RevRobotics20HDHex;
        /// 40:1 spur gearbox hex motor.
        pub struct RevRobotics40HDHex;
        /// Rev robotics' core hex motor.
        pub struct RevRoboticsCoreHex;

        /// 53:1 ratio spur gear motor.
        pub struct GoBilda5201;
        /// 6mm D, 24mm length shaft planetary motor.
        pub struct GoBilda5202;
        /// 8mm REX, 24mm length shaft planetary motor. Same as the GoBilda5202 in the FTC SDK.
        pub type GoBilda5203 = GoBilda5202;
        /// 8mm REX, 80mm length shaft planetary motor. Same as the GoBilda5202 in the FTC SDK.
        pub type GoBilda5204 = GoBilda5202;

        /// I can't find any information on this motor online, other then the part number W39530.
        pub struct Tetrix;
    }

    /// A servo motor. For continuous servos, look at [`CRServo`].
    pub struct Servo;
    /// A continuous servo motor that doesn't have defined stops.
    pub struct CRServo;
    /// The REV spark mini motor controller. Shows up as a [`DcMotorSimple`](crate::hardware::DcMotorSimple) in code.
    pub struct RevSPARKMini;

    /// The IMU embedded in the control hub. Cannot be used under an expansion hub.
    pub struct EmbeddedIMU;

    /// The Limelight Vision Limelight 3A Vision Sensor. Provides camera functionality.
    pub struct Limelight3A;

    /// The control hub.
    pub static CTRL_HUB: () = ();
    /// The expansion hub.
    pub static EXP_HUB: () = ();

    /// The name of the configuration.
    pub static CONFIG_NAME: &str = "";
    /// The name of the FTC crate.
    pub static FTC_NAME: &str = "";
    /// Whether an expansion hub exists in this configuration.
    pub static HAS_EXP_HUB: bool = false;
}

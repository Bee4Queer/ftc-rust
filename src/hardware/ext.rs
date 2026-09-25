//! Extension device types that don't exist in the regular Java SDK.

use crate::{device, hardware::Servo};

device!(
    /// Gobuild LED that pretends to be a servo. <https://www.gobilda.com/rgb-indicator-light-pwm-controlled>
    GobuildaRGBIndicatorLight wraps Servo,
);

impl GobuildaRGBIndicatorLight {
    /// Set the color of the light.
    #[inline(always)]
    pub fn set_color(&self, color: GobuildaServoLedColor) {
        self.inner.set_target_position(color.position());
    }
}

/// convenience macro for `GobuildaRGBIndicatorLight` stuff
macro_rules! gobuild_rgb_indicator_light {
    ($($color:ident => $value:expr),* $(,)?) => {
        pastey::paste!{
            impl GobuildaRGBIndicatorLight {
                $(
                    #[doc = concat!("Set the color of this light to ", stringify!($color), ".")]
                    #[inline(always)]
                    pub fn [< $color:snake >] (&self) {
                        self.set_color(GobuildaServoLedColor::$color)
                    }
                )*
            }
        }

        #[allow(missing_docs)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum GobuildaServoLedColor {
            $($color),*
        }

        impl GobuildaServoLedColor {
            /// The "servo" position to set to for a certain color.
            #[must_use]
            pub fn position(self) -> f64 {
                match self {
                    $(Self::$color => $value),*
                }
            }
        }
    };
}

gobuild_rgb_indicator_light!(
    Off => 0.0,
    Red => 0.277,
    Orange => 0.333,
    Yellow => 0.388,
    Sage => 0.444,
    Green => 0.5,
    Azure => 0.555,
    Blue => 0.611,
    Indigo => 0.666,
    Violet => 0.722,
    White => 1.0,
);

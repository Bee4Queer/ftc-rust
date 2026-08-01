//! Hardware sensors.

use crate::{call_method, get_field, hardware::IntoJniObject};

device!(
    /// Javadoc available at <https://javadoc.io/doc/org.firstinspires.ftc/RobotCore/latest/com/qualcomm/robotcore/hardware/NormalizedColorSensor.html>.
    ///
    /// Wrapper around the `NormalizedColorSensor` class, not the `ColorSensor` class. This includes all officially supported color sensors.
    ColorSensor,
    JAVA_CLASS = "com.qualcomm.robotcore.hardware.NormalizedColorSensor";
    JNI_CLASS = "com/qualcomm/robotcore/hardware/NormalizedColorSensor";
);

/// Colors read from a [`ColorSensor`].
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
#[doc(alias = "NormalizedRGBA")]
pub struct Colors {
    /// The red component.
    #[doc(alias = "red")]
    pub r: f32,
    /// The green component.
    #[doc(alias = "green")]
    pub g: f32,
    /// The blue component.
    #[doc(alias = "blue")]
    pub b: f32,
    /// The alpha component.
    #[doc(alias = "alpha")]
    pub a: f32,
}

impl IntoJniObject for Colors {
    const JAVA_CLASS: &'static str = "com.qualcomm.robotcore.hardware.NormalizedRGBA";
    const JNI_CLASS: &'static str = "com/qualcomm/robotcore/hardware/NormalizedRGBA";
    fn into_jni_object<'local>(self, _env: &mut jni::Env<'local>) -> jni::objects::JObject<'local> {
        unimplemented!()
    }
    fn from_jni_object(
        vm: &jni::vm::JavaVM,
        obj: jni::refs::Global<jni::objects::JObject<'static>>,
    ) -> Self {
        vm.attach_current_thread(|env| {
            jni::errors::Result::Ok(Colors {
                r: get_field!(float env, obj, "red"),
                g: get_field!(float env, obj, "green"),
                b: get_field!(float env, obj, "blue"),
                a: get_field!(float env, obj, "alpha"),
            })
        })
        .unwrap()
    }
}

impl ColorSensor {
    /// Reads the colors from the sensor.
    #[must_use]
    #[doc(alias = "getNormalizedColors")]
    pub fn colors(&self) -> Colors {
        Colors::from_jni_object(
            &self.vm,
            call_method!(obj self, self.object, "getNormalizedColors", "()Lcom/qualcomm/robotcore/hardware/NormalizedRGBA;", []),
        )
    }
    /// Get the current gain of the sensor.
    #[must_use]
    #[doc(alias = "getGain")]
    pub fn gain(&self) -> f32 {
        call_method!(float self, self.object, "getGain", "()F", [])
    }
    /// Get the current gain of the sensor.
    #[doc(alias = "setGain")]
    pub fn set_gain(&self, gain: f32) {
        call_method!(void self, self.object, "setGain", "(F)V", [gain]);
    }
}

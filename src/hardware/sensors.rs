//! Hardware sensors.

use jni::{jni_sig, jni_str};

use crate::{call_method, device, get_field, hardware::IntoJniObject};

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

device!(
    /// Javadoc available at <https://javadoc.io/doc/org.firstinspires.ftc/RobotCore/latest/com/qualcomm/robotcore/hardware/LightSensor.html>.
    ///
    /// Basic light sensor.
    LightSensor,
    JAVA_CLASS = "com.qualcomm.robotcore.hardware.LightSensor";
    JNI_CLASS = "com/qualcomm/robotcore/hardware/LightSensor";
);

impl LightSensor {
    /// Get the amount of light detected by the sensor, scaled and cliped to a rangewhich is a
    /// pragmatically useful sensitivity.
    #[doc(alias = "getLightDetected")]
    #[must_use]
    pub fn adjusted_light(&self) -> f64 {
        call_method!(double self, self.object, "getLightDetected", "()D", [])
    }
    /// Returns a signal whose strength is proportional to the intensity of the light measured.Note
    /// that returned values INCREASE as the light energy INCREASES.
    #[doc(alias = "getRawLightDetected")]
    #[must_use]
    pub fn raw_light(&self) -> f64 {
        call_method!(double self, self.object, "getRawLightDetected", "()D", [])
    }
    /// Returns the maximum value that can be returned by [`Self::raw_light`].
    #[doc(alias = "getRawLightDetectedMax")]
    #[must_use]
    pub fn raw_light_max(&self) -> f64 {
        call_method!(double self, self.object, "getRawLightDetectedMax", "()D", [])
    }
    /// Enable the LED light.
    #[doc(alias = "enableLed")]
    pub fn enable_led(&self) {
        call_method!(void self, self.object, "enableLed", "(B)V", [true]);
    }
    /// Disable the LED light.
    #[doc(alias = "enableLed")]
    pub fn disable_led(&self) {
        call_method!(void self, self.object, "disableLed", "(B)V", [false]);
    }
}

device!(
    /// Javadoc available at <https://javadoc.io/doc/org.firstinspires.ftc/RobotCore/latest/com/qualcomm/robotcore/hardware/DistanceSensor.html>.
    ///
    /// The `DistanceSensor` may be found on hardware sensors which measure distance by one means or another.
    DistanceSensor,
    JAVA_CLASS = "com.qualcomm.robotcore.hardware.DistanceSensor";
    JNI_CLASS = "com/qualcomm/robotcore/hardware/DistanceSensor";
);

impl DistanceSensor {
    /// Returns the current distance in millimeters.
    #[doc(alias = "getDistance")]
    #[must_use]
    pub fn get_distance(&self) -> f64 {
        self.vm.attach_current_thread(|env| {
            let unit_class = env.find_class(
                jni_str!("org/firstinspires/ftc/robotcore/external/navigation/DistanceUnit")
            )?;
            let unit_mm = env.get_static_field(
                unit_class,
                jni_str!("MM"),
                jni_sig!("Lorg/firstinspires/ftc/robotcore/external/navigation/DistanceUnit;")
            )?;

            call_method!(env env, &self.object, "getDistance", "(Lorg/firstinspires/ftc/robotcore/external/navigation/DistanceUnit;)D", [&unit_mm])?.d()
        }).unwrap()
    }
}

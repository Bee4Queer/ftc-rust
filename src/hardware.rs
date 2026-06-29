//! Hardware-related code.
#![allow(clippy::needless_pass_by_value)]

use std::{
    any::type_name,
    fmt::{Debug, Display},
};

use jni::{
    Env, JavaVM, jni_sig,
    objects::{JClass, JObject, JString},
    refs::Global,
    signature::{RuntimeFieldSignature, RuntimeMethodSignature},
    strings::JNIString,
};

mod devices;
pub use devices::*;
use log::{error, trace};

use crate::{call_method, new_global, new_string};

/// A device that can be made from a java object.
pub trait Device: Default {
    /// Create a new instance of this type from the java environment and the relevant object.
    fn from_java(vm: JavaVM, object: Global<JObject<'static>>) -> Self;
    /// The Java-formatted class name. Unlike other JNI things, this uses dots.
    const JAVA_CLASS: &'static str;
    /// The JNI-formatted class name. Uses forward slashes instead of dots.
    const JNI_CLASS: &'static str;
}

/// A wrapper for accessing hardware-related methods.
#[doc(alias = "HardwareMap")]
#[must_use]
pub struct Hardware {
    /// The environment.
    pub(crate) vm: JavaVM,
    /// The actual hardwareMap object. Should be com/qualcomm/robotcore/hardware/HardwareMap.
    pub(crate) hardware_map: Global<JObject<'static>>,
}

impl Debug for Hardware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("(opaque Hardware object)")
    }
}

impl Hardware {
    /// Get a [`Device`] from the hardware map.
    pub fn get<T: Device>(&self, name: impl AsRef<str>) -> T {
        trace!(
            "getting device `{}` of type `{}`",
            name.as_ref(),
            type_name::<T>()
        );
        let object = self
            .vm
            .attach_current_thread(|env| {
                let class = env.load_class(JNIString::new(T::JAVA_CLASS)).unwrap();
                let jname = new_string!(env env, name.as_ref()).unwrap();
                trace!("arguments prepared");

                let res = env.call_method(
                    &self.hardware_map,
                    JNIString::new("get"),
                    jni_sig!("(Ljava/lang/Class;Ljava/lang/String;)Ljava/lang/Object;"),
                    &[(&class).into(), (&jname).into()],
                );

                trace!("called method");

                match res {
                    Ok(res) => new_global!(env, res.l().unwrap()),
                    Err(err) => {
                        error!("Got error {err} trying to get device `{}`!", name.as_ref());
                        Err(err)
                    }
                }
            })
            .map_err(|err| panic!("{err}")) // panic payloads are weird
            .unwrap();
        trace!("got device");

        T::from_java(self.vm.clone(), object)
    }
}

impl Iterator for Hardware {
    type Item = HardwareDevice;
    fn next(&mut self) -> Option<Self::Item> {
        self.vm
            .attach_current_thread(|env| {
                call_method!(
                    env env,
                    self.hardware_map,
                    "next",
                    format!("()L{};", HardwareDevice::JNI_CLASS),
                    []
                )
                .map(|v| HardwareDevice {
                    vm: self.vm.clone(),
                    hardware_device: new_global!(env, v.l().unwrap()).unwrap(),
                })
            })
            .ok()
    }
}

/// Get a `JClass` of the provided type.
fn get_class<'local>(env: &mut Env<'local>, jni_class: impl AsRef<str>) -> JClass<'local> {
    env.load_class(JNIString::new(jni_class)).unwrap()
}

/// Generate an implementation of `IntoJniObject` for an enum.
macro_rules! enum_variant_into {
    {
        $vis:vis body, $jni_class:literal,
        $java_class:literal,
        $($variant:ident),*
        $(; PREFIX = $prefix:literal)?
        $(; suffix = $suffix:literal)?
        $(,)?
        $(;)?
    } => {
        paste::paste!{
            /// JNI class
            $vis const [< $($prefix)? JNI_CLASS >]: &'static str = $jni_class;
            /// Java class
            $vis const [< $($prefix)? JAVA_CLASS >]: &'static str = $java_class;
            /// conversion
            $vis fn [<into_jni_object $($suffix)?>]<'local>(self, env: &mut Env<'local>) -> JObject<'local> {
                let class = get_class(env, Self:: [< $($prefix)? JNI_CLASS >]);
                env
                    .get_static_field(
                        class,
                        JNIString::new(match self {
                            $(Self:: $variant => stringify!($variant).to_uppercase()),*
                        }),
                        RuntimeFieldSignature::from_str(concat!("L", $jni_class, ";")).unwrap().field_signature(),
                    )
                    .unwrap()
                    .l()
                    .unwrap()
            }

            /// conversion
            $vis fn [<from_jni_object $($suffix)?>](
                vm: &JavaVM,
                obj: Global<JObject<'static>>,
            ) -> Self {
                let res = vm.attach_current_thread(|env| call_method!(env env, obj, "ordinal", "()I", []).unwrap().i()).unwrap();
                let mut items = vec![$(Self:: $variant),*];
                let full_len = items.len();
                match res {
                    $(x if x == {items.pop(); (full_len - items.len()) as i32} => Self:: $variant),*,
                    _ => unreachable!()
                }
            }
        }
    };
    {
        $ty:ty,
        $jni_class:literal,
        $java_class:literal,
        $($variant:ident),*
        $(,)?
    } => {
        impl IntoJniObject for $ty {
            enum_variant_into!(body, $jni_class, $java_class, $($variant),*);
        }
    };
}

/// Convert this type into a JNI object.
pub trait IntoJniObject {
    /// The JNI-formatted class name.
    const JNI_CLASS: &'static str;
    /// The Java-formatted class name.
    const JAVA_CLASS: &'static str;

    /// Convert this type into a `JObject`.
    fn into_jni_object<'local>(self, env: &mut Env<'local>) -> JObject<'local>;
    /// Convert a `JObject` into this type.
    fn from_jni_object(vm: &JavaVM, obj: Global<JObject<'static>>) -> Self;
}

/// `DcMotor`s can be configured to internally reverse the values to which, e.g., their motor power
/// is set. This makes it easy to have drive train motors on two sides of a robot: during
/// initialization, one would be set at at forward, the other at reverse, and the difference between
/// the two in that respect could be there after ignored.
///
/// At the start of an `OpMode`, motors are guaranteed to be in the forward direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[must_use]
pub enum Direction {
    /// Turn forward. Commonly clockwise.
    #[default]
    Forward,
    /// Turn backward. Commonly counterclockwise.
    Reverse,
}

enum_variant_into! {
    Direction,
    "com/qualcomm/robotcore/hardware/DcMotorSimple$Direction",
    "com.qualcomm.robotcore.hardware.DcMotorSimple.Direction",
    Forward,
    Reverse,
}

impl Direction {
    /// The JNI class for a servo direction.
    pub const SERVO_JNI_CLASS: &'static str = "com/qualcomm/robotcore/hardware/Servo$Direction";
    /// The Java class for a servo direction.
    pub const SERVO_JAVA_CLASS: &'static str = "com.qualcomm.robotcore.hardware.Servo.Direction";
    /// Convert this to a JNI object for a `Servo` as it for some reason uses a different type.
    pub fn into_jni_object_servo<'local>(self, env: &mut Env<'local>) -> JObject<'local> {
        let class = get_class(env, Self::SERVO_JNI_CLASS);
        env.get_static_field(
            class,
            JNIString::new(match self {
                Self::Forward => "Forward".to_uppercase(),
                Self::Reverse => "Reverse".to_uppercase(),
            }),
            RuntimeFieldSignature::from_str(Self::SERVO_JNI_CLASS)
                .unwrap()
                .field_signature(),
        )
        .unwrap()
        .l()
        .unwrap()
    }
    /// Convert this from a JNI object for a `Servo` as it for some reason uses a different type.
    pub fn from_jni_object_servo(vm: &JavaVM, obj: Global<JObject<'static>>) -> Self {
        let res = vm
            .attach_current_thread(|env| {
                {
                    let env: &mut crate::jni::Env = env;
                    let obj = env.new_local_ref(&obj).unwrap();
                    env.call_method(
                        &obj,
                        crate::jni::strings::JNIString::new("ordinal"),
                        jni_sig!("()I"),
                        &[],
                    )
                }
                .unwrap()
                .i()
            })
            .unwrap();
        let mut items = vec![Self::Forward, Self::Reverse];
        let full_len = items.len();
        match res {
            x if x == {
                items.pop();
                (full_len - items.len()) as i32
            } =>
            {
                Self::Forward
            }
            x if x == {
                items.pop();
                (full_len - items.len()) as i32
            } =>
            {
                Self::Reverse
            }
            _ => unreachable!(),
        }
    }
}

/// `ZeroPowerBehavior` provides an indication as to a motor's behavior when a power level of zero
/// is applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[must_use]
pub enum ZeroPowerBehavior {
    /// The behavior of the motor when zero power is applied is not currently known. This value is
    /// mostly useful for your internal state variables. It may not be passed as a parameter to
    /// `set_zero_power_behavior` and will never be returned from `get_zero_power_behavior`.
    Unknown,
    /// The motor stops and then brakes, actively resisting any external force which attempts to
    /// turn the motor.
    Brake,
    /// The motor stops and then floats: an external force attempting to turn the motor is not met
    /// with active resistance.
    Float,
}

enum_variant_into! {
    ZeroPowerBehavior,
    "com/qualcomm/robotcore/hardware/DcMotor$ZeroPowerBehavior",
    "com.qualcomm.robotcore.hardware.DcMotor.ZeroPowerBehavior",
    Unknown,
    Brake,
    Float,
}

/// The run mode of a motor controls how the motor interprets its parameter settings passed through
/// power- and encoder-related methods. Some of these modes internally use `PIDcontrol` to achieve
/// their function, while others do not. Those that do are referred to as "PID modes".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[must_use]
pub enum RunMode {
    /// The motor is simply to run at whatever velocity is achieved by apply a particular power
    /// level to the motor.
    RunWithoutEncoder,
    /// The motor is to do its best to run at targeted velocity. An encoder must be affixed to the
    /// motor in order to use this mode. This is a PID mode.
    RunUsingEncoder,
    /// The motor is to attempt to rotate in whatever direction is necessary to cause the encoder
    /// reading to advance or retreat from its current setting to the setting which has been
    /// provided through the `set_target_position` method. An encoder must be affixed to this
    /// motor in order to use this mode. This is a PID mode.
    RunToPosition,
    /// The motor is to set the current encoder position to zero. In contrast to `RunToPosition`,
    /// the motor is not rotated in order to achieve this; rather, the current rotational
    /// position of the motor is simply reinterpreted as the new zero value. However, as a side
    /// effect of placing a motor in this mode, power is removed from the motor, causing it to
    /// stop, though it is unspecified whether the motor enters brake or float mode. Further, it
    /// should be noted that setting a motor to `StopAndResetEncoder` may or may not be a transient
    /// state: motors connected to some motor controllers will remain in this mode until
    /// explicitly transitioned to a different one, while motors connected to other motor
    /// controllers will automatically transition to a different mode after the reset of the encoder
    /// is complete.
    StopAndResetEncoder,
}

enum_variant_into! {
    RunMode,
    "com/qualcomm/robotcore/hardware/DcMotor$RunMode",
    "com.qualcomm.robotcore.hardware.DcMotor.RunMode",
    RunWithoutEncoder,
    RunUsingEncoder,
    RunToPosition,
    StopAndResetEncoder,
}

/// Angle units.
#[allow(missing_docs, reason = "angle units don't need to be explained")]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum AngleUnit {
    Degree,
    Radian,
}

impl Debug for AngleUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl Display for AngleUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Degree => "°",
            Self::Radian => "rad",
        })
    }
}

enum_variant_into! {
    AngleUnit,
    "org/firstinspires/ftc/robotcore/external/navigation/AngleUnit",
    "org.firstinspires.ftc.robotcore.external.navigation.AngleUnit",
    Degree,
    Radian,
}

impl AngleUnit {
    /// Convert a value from this unit to another.
    #[must_use]
    pub fn to_unit(self, to: Self, value: f64) -> f64 {
        match (self, to) {
            (AngleUnit::Degree, AngleUnit::Degree) | (AngleUnit::Radian, AngleUnit::Radian) => {
                value
            }
            (AngleUnit::Degree, AngleUnit::Radian) => value.to_radians(),
            (AngleUnit::Radian, AngleUnit::Degree) => value.to_degrees(),
        }
    }
    /// Convert a value from this unit to another.
    #[must_use]
    pub fn to_unit_f32(self, to: Self, value: f32) -> f32 {
        match (self, to) {
            (AngleUnit::Degree, AngleUnit::Degree) | (AngleUnit::Radian, AngleUnit::Radian) => {
                value
            }
            (AngleUnit::Degree, AngleUnit::Radian) => value.to_radians(),
            (AngleUnit::Radian, AngleUnit::Degree) => value.to_degrees(),
        }
    }
    enum_variant_into! {
        pub body,
        "org/firstinspires/ftc/robotcore/external/navigation/UnnormalizedAngleUnit",
        "org.firstinspires.ftc.robotcore.external.navigation.UnnormalizedAngleUnit",
        Degree,
        Radian;
        PREFIX = "UNNORMALIZED_";
        suffix = "_unnormalized";
    }
}

/// Instances of `AngularVelocity` represent an instantaneous body-referenced 3D rotation rate.
///
/// The instantaneous rate of change of an Orientation, which is what we are representing here, has
/// unexpected subtleties. As described in Section 9.3 of the MIT Kinematics Lecture below, the
/// instantaneous body-referenced rotation rate angles are decoupled (their order does not matter)
/// but their conversion into a corresponding instantaneous rate of change of a set of related Euler
/// angles (ie: Orientation involves a non-obvious transformation on two of the rotation rates.
///
/// The speeds are unnormalized angles, meaning they can be outside of the range -180..=180 if
/// degrees or -π..=π if radians.
#[derive(Clone, Copy, PartialEq)]
#[must_use]
#[allow(
    missing_docs,
    clippy::missing_docs_in_private_items,
    reason = "explained in type docs"
)]
pub struct AngularVelocity {
    pub x_speed: f32,
    pub y_speed: f32,
    pub z_speed: f32,
    pub acquisition_time: i64,
    /// The unit of these speeds.
    pub unit: AngleUnit,
}

impl AngularVelocity {
    /// Convert this angular velocity to another unit.
    pub fn to_unit(self, unit: AngleUnit) -> Self {
        Self {
            x_speed: self.unit.to_unit_f32(unit, self.x_speed),
            y_speed: self.unit.to_unit_f32(unit, self.y_speed),
            z_speed: self.unit.to_unit_f32(unit, self.z_speed),
            acquisition_time: self.acquisition_time,
            unit,
        }
    }
}

impl IntoJniObject for AngularVelocity {
    const JAVA_CLASS: &'static str =
        "org.firstinspires.ftc.robotcore.external.navigation.AngularVelocity";
    const JNI_CLASS: &'static str =
        "org/firstinspires/ftc/robotcore/external/navigation/AngularVelocity";

    fn into_jni_object<'local>(self, env: &mut Env<'local>) -> JObject<'local> {
        let class = get_class(env, Self::JNI_CLASS);

        let angle = self.unit.into_jni_object(env);

        env.new_object(
            class,
            RuntimeMethodSignature::from_str(format!(
                "(L{};FFFJ)L{};",
                AngleUnit::JNI_CLASS,
                Self::JNI_CLASS
            ))
            .unwrap()
            .method_signature(),
            &[
                (&angle).into(),
                self.x_speed.into(),
                self.y_speed.into(),
                self.z_speed.into(),
                self.acquisition_time.into(),
            ],
        )
        .unwrap()
    }
    fn from_jni_object(vm: &JavaVM, obj: Global<JObject<'static>>) -> Self {
        vm.attach_current_thread(|env| {
            let x_speed = env
                .get_field(&obj, JNIString::new("xRotationRate"), jni_sig!("F"))
                .unwrap()
                .f()
                .unwrap();
            let y_speed = env
                .get_field(&obj, JNIString::new("yRotationRate"), jni_sig!("F"))
                .unwrap()
                .f()
                .unwrap();
            let z_speed = env
                .get_field(&obj, JNIString::new("zRotationRate"), jni_sig!("F"))
                .unwrap()
                .f()
                .unwrap();

            let acquisition_time = env
                .get_field(&obj, JNIString::new("acquisitionTime"), jni_sig!("J"))
                .unwrap()
                .j()
                .unwrap();

            let unit = env
                .get_field(
                    &obj,
                    JNIString::new("angleUnit"),
                    RuntimeFieldSignature::from_str(format!(
                        "L{};",
                        AngleUnit::UNNORMALIZED_JNI_CLASS
                    ))
                    .unwrap()
                    .field_signature(),
                )
                .unwrap()
                .l()
                .unwrap();

            let unit = AngleUnit::from_jni_object_unnormalized(vm, new_global!(env, unit).unwrap());

            jni::errors::Result::Ok(AngularVelocity {
                x_speed,
                y_speed,
                z_speed,
                acquisition_time,
                unit,
            })
        })
        .unwrap()
    }
}

impl Debug for AngularVelocity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as Display>::fmt(self, f)
    }
}

impl Display for AngularVelocity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{{x={:.3}{3}, y={:.3}{3}, z={:.3}{3}}}",
            self.x_speed, self.y_speed, self.z_speed, self.unit
        ))
    }
}

/// A simplified view of the orientation of an object in 3D space.
///
/// Yaw is side-to-side lateral rotation, where the object remains flat, but turns left and right.
/// Sometimes yaw is also referred to as "heading".
///
/// Pitch is front-to-back rotation, where the front of the object moves upwards while the rear of
/// the object moves downwards, or vice versa.
///
/// Roll is side-to-side tilt, where the left side of the object moves upwards while the right side
/// of the object moves downwards, or vice versa.
///
/// All angles are in the range of -180 degrees to 180 degrees.
///
/// The angles are applied intrinsically, in the order of yaw, then pitch, then roll.
/// "Intrinsically" means that the axes move along with the object as you perform the rotations. As
/// an example using a robot, if the yaw is 30 degrees, the pitch is 40 degrees, and the roll is 10
/// degrees, that means that you would reach the described orientation by first rotating the object
/// 30 degrees counter-clockwise from the starting point, with all wheels continuing to touch the
/// ground (rotation around the Z axis, as defined in the Robot Coordinate System). Then, you make
/// your robot point 40 degrees upward (rotate it 40 degrees around the X axis, as defined in the
/// Robot Coordinate System). Because the X axis moved with the robot, the pitch is not affected by
/// the yaw value. Then from that position, the robot is tilted 10 degrees to the right, around the
/// newly positioned Y axis, to produce the actual position of the robot.
#[derive(Clone, Copy, PartialEq)]
#[must_use]
#[allow(
    missing_docs,
    clippy::missing_docs_in_private_items,
    reason = "explained in type docs"
)]
pub struct YawPitchRollAngles {
    yaw: f64,
    pitch: f64,
    roll: f64,
    pub acquisition_time: i64,
    /// The unit of these angles.
    unit: AngleUnit,
}

impl YawPitchRollAngles {
    /// Create a new instance.
    pub fn new(unit: AngleUnit, yaw: f64, pitch: f64, roll: f64, acquisition_time: i64) -> Self {
        Self {
            yaw,
            pitch,
            roll,
            acquisition_time,
            unit,
        }
    }
    /// Get the yaw.
    #[must_use]
    pub fn yaw(&self, unit: AngleUnit) -> f64 {
        self.unit.to_unit(unit, self.yaw)
    }
    /// Get the pitch.
    #[must_use]
    pub fn pitch(&self, unit: AngleUnit) -> f64 {
        self.unit.to_unit(unit, self.pitch)
    }
    /// Get the roll.
    #[must_use]
    pub fn roll(&self, unit: AngleUnit) -> f64 {
        self.unit.to_unit(unit, self.roll)
    }

    /// Set the yaw.
    pub fn set_yaw(&mut self, unit: AngleUnit, value: f64) {
        self.yaw = unit.to_unit(self.unit, value);
    }
    /// Set the pitch.
    pub fn set_pitch(&mut self, unit: AngleUnit, value: f64) {
        self.pitch = unit.to_unit(self.unit, value);
    }
    /// Set the roll.
    pub fn set_roll(&mut self, unit: AngleUnit, value: f64) {
        self.roll = unit.to_unit(self.unit, value);
    }
    /// Ensures that the angles are within the range -180..=180.
    #[must_use]
    pub fn validate(&self) -> bool {
        let range = -180f64..=180f64;
        range.contains(&self.yaw(AngleUnit::Degree))
            && range.contains(&self.pitch(AngleUnit::Degree))
            && range.contains(&self.roll(AngleUnit::Degree))
    }
}

impl IntoJniObject for YawPitchRollAngles {
    const JAVA_CLASS: &'static str =
        "org.firstinspires.ftc.robotcore.external.navigation.YawPitchRollAngles";
    const JNI_CLASS: &'static str =
        "org/firstinspires/ftc/robotcore/external/navigation/YawPitchRollAngles";

    fn into_jni_object<'local>(self, env: &mut Env<'local>) -> JObject<'local> {
        debug_assert!(self.validate());

        let class = get_class(env, Self::JNI_CLASS);

        let angle = self.unit.into_jni_object(env);

        env.new_object(
            class,
            RuntimeMethodSignature::from_str(format!(
                "(L{};DDDJ)L{};",
                AngleUnit::JNI_CLASS,
                Self::JNI_CLASS
            ))
            .unwrap()
            .method_signature(),
            &[
                (&angle).into(),
                self.yaw.into(),
                self.pitch.into(),
                self.roll.into(),
                self.acquisition_time.into(),
            ],
        )
        .unwrap()
    }
    fn from_jni_object(vm: &JavaVM, obj: Global<JObject<'static>>) -> Self {
        vm.attach_current_thread(|env| {
            let unit = AngleUnit::Degree.into_jni_object(env);

            let yaw = call_method!(env env, obj, "getYaw", format!("(L{};)D", AngleUnit::JNI_CLASS), [&unit])
                .unwrap()
                .d()
                .unwrap();
            let pitch = call_method!(env env, obj, "getPitch", format!("(L{};)D", AngleUnit::JNI_CLASS), [&unit])
                .unwrap()
                .d()
                .unwrap();
            let roll = call_method!(env env, obj, "getRoll", format!("(L{};)D", AngleUnit::JNI_CLASS), [&unit])
                .unwrap()
                .d()
                .unwrap();

            let acquisition_time = call_method!(env env, obj, "getAcquisitionTime", "()J", []).unwrap().j().unwrap();

            jni::errors::Result::Ok(
                YawPitchRollAngles { yaw, pitch, roll, acquisition_time, unit: AngleUnit::Degree }
            )
        }).unwrap()
    }
}

impl Debug for YawPitchRollAngles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as Display>::fmt(self, f)
    }
}

impl Display for YawPitchRollAngles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{{yaw={:.3}°, pitch={:.3}°, roll={:.3}°}}",
            self.yaw(AngleUnit::Degree),
            self.pitch(AngleUnit::Degree),
            self.roll(AngleUnit::Degree)
        ))
    }
}

/// The orientation of something on the robot.
#[allow(missing_docs, reason = "orientations don't need to be explained")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[must_use]
pub enum Orientation {
    Up,
    Down,
    Forward,
    Backward,
    Left,
    Right,
}

impl Orientation {
    enum_variant_into! {
        pub body,
        "com/qualcomm/hardware/rev/Rev9AxisImuOrientationOnRobot$LogoFacingDirection",
        "com.qualcomm.hardware.rev.Rev9AxisImuOrientationOnRobot.LogoFacingDirection",
        Up,
        Down,
        Forward,
        Backward,
        Left,
        Right;
        PREFIX = "LOGO_";
        suffix = "_logo";
    }
    enum_variant_into! {
        pub body,
        "com/qualcomm/hardware/rev/Rev9AxisImuOrientationOnRobot$I2cPortFacingDirection",
        "com.qualcomm.hardware.rev.Rev9AxisImuOrientationOnRobot.I2cPortFacingDirection",
        Up,
        Down,
        Forward,
        Backward,
        Left,
        Right;
        PREFIX = "I2C_";
        suffix = "_i2c";
    }
}

/// The orientation at which a given REV External IMU is mounted to a robot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[must_use]
pub struct Rev9AxisImuOrientationOnRobot {
    /// The direction of the logo on the robot.
    pub logo_dir: Orientation,
    /// The direction of the i2c port on the robot.
    pub i2c_dir: Orientation,
}

impl Rev9AxisImuOrientationOnRobot {
    /// Validate this orientation.
    #[must_use]
    pub fn validate(&self) -> bool {
        match self.logo_dir {
            Orientation::Up | Orientation::Down => match self.i2c_dir {
                Orientation::Up | Orientation::Down => false,
                Orientation::Forward
                | Orientation::Backward
                | Orientation::Left
                | Orientation::Right => true,
            },
            Orientation::Forward => match self.i2c_dir {
                Orientation::Forward | Orientation::Backward => false,
                Orientation::Up | Orientation::Down | Orientation::Left | Orientation::Right => {
                    true
                }
            },
            Orientation::Backward => match self.i2c_dir {
                Orientation::Forward | Orientation::Backward => false,
                Orientation::Up | Orientation::Down | Orientation::Left | Orientation::Right => {
                    true
                }
            },
            Orientation::Left | Orientation::Right => match self.i2c_dir {
                Orientation::Up
                | Orientation::Down
                | Orientation::Forward
                | Orientation::Backward => true,
                Orientation::Left | Orientation::Right => false,
            },
        }
    }
}

impl IntoJniObject for Rev9AxisImuOrientationOnRobot {
    const JAVA_CLASS: &'static str = "com.qualcomm.hardware.rev.Rev9AxisImuOrientationOnRobot";
    const JNI_CLASS: &'static str = "com/qualcomm/hardware/rev/Rev9AxisImuOrientationOnRobot";
    fn into_jni_object<'local>(self, env: &mut Env<'local>) -> JObject<'local> {
        let logo = self.logo_dir.into_jni_object_logo(env);
        let i2c = self.i2c_dir.into_jni_object_i2c(env);

        let class = get_class(env, Self::JNI_CLASS);

        env.new_object(
            class,
            RuntimeMethodSignature::from_str(format!(
                "(L{};L{};)L{};",
                Orientation::LOGO_JNI_CLASS,
                Orientation::I2C_JNI_CLASS,
                Self::JNI_CLASS
            ))
            .unwrap()
            .method_signature(),
            &[(&logo).into(), (&i2c).into()],
        )
        .unwrap()
    }
    fn from_jni_object(_: &JavaVM, _: Global<JObject<'static>>) -> Self {
        unimplemented!(
            "I'm honestly not sure if it's possible to convert a `Rev9AxisImuOrientationOnRobot` \
             from Java. If you know either way, make a PR to {}:{}!",
            file!(),
            line!()
        )
    }
}

/// The manufacturer of a part.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[allow(missing_docs, reason = "manufacturer names")]
#[must_use]
pub enum Manufacturer {
    #[default]
    Unknown,
    Other,
    Lego,
    HiTechnic,
    ModernRobotics,
    Adafruit,
    Matrix,
    Lynx,
    AMS,
    STMicroelectronics,
    Broadcom,
    DFRobot,
    DigitalChickenLabs,
    SparkFun,
    MaxBotix,
    LimelightVision,
    GoBilda,
}

impl Display for Manufacturer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Manufacturer::Unknown => "Unknown Vendor",
            Manufacturer::Other => "Other Vendor",
            Manufacturer::Lego => "LEGO",
            Manufacturer::HiTechnic => "HiTechnic",
            Manufacturer::ModernRobotics => "Modern Robotics",
            Manufacturer::Adafruit => "Adafruit",
            Manufacturer::Matrix => "Matrix",
            Manufacturer::Lynx => "Lynx",
            Manufacturer::AMS => "AMS",
            Manufacturer::STMicroelectronics => "ST Microelectronics",
            Manufacturer::Broadcom => "Broadcom",
            Manufacturer::DFRobot => "DFRobot",
            Manufacturer::DigitalChickenLabs => "Digital Chicken Labs",
            Manufacturer::SparkFun => "SparkFun Electronics",
            Manufacturer::MaxBotix => "MaxBotix",
            Manufacturer::LimelightVision => "Limelight",
            Manufacturer::GoBilda => "GoBilda",
        })
    }
}

enum_variant_into! {
    Manufacturer,
    "com/qualcomm/robotcore/hardware/HardwareDevice$Manufacturer",
    "com.qualcomm.robotcore.hardware.HardwareDevice.Manufacturer",
    Unknown,
    Other,
    Lego,
    HiTechnic,
    ModernRobotics,
    Adafruit,
    Matrix,
    Lynx,
    AMS,
    STMicroelectronics,
    Broadcom,
    DFRobot,
    DigitalChickenLabs,
    SparkFun,
    MaxBotix,
    LimelightVision,
    GoBilda,
}

/// A hardware device.
pub struct HardwareDevice {
    /// The environment.
    vm: JavaVM,
    /// The actual device object. Should be com/qualcomm/robotcore/hardware/HardwareDevice.
    hardware_device: Global<JObject<'static>>,
}

impl Debug for HardwareDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HardwareDevice")
            .field("manufacturer", &self.get_manufacturer())
            .field("device_name", &self.get_device_name())
            .field("connection_info", &self.get_connection_info())
            .finish()
    }
}

impl HardwareDevice {
    /// The class in java of this type.
    pub const JAVA_CLASS: &'static str = "com.qualcomm.robotcore.hardware.HardwareDevice";
    /// The class in the JNI of this type.
    pub const JNI_CLASS: &'static str = "com/qualcomm/robotcore/hardware/HardwareDevice";

    /// Returns an indication of the manufacturer of this device.
    #[doc(alias = "getManufacturer")]
    pub fn get_manufacturer(&self) -> Manufacturer {
        self.vm
            .attach_current_thread(|env| {
                let res = call_method!(
                    env env,
                    self.hardware_device,
                    "getManufacturer",
                    format!("()L{};", Manufacturer::JNI_CLASS),
                    []
                )
                .unwrap()
                .l()
                .unwrap();
                jni::errors::Result::Ok(Manufacturer::from_jni_object(
                    &self.vm,
                    new_global!(env, res).unwrap(),
                ))
            })
            .unwrap()
    }
    /// Returns a string suitable for display to the user as to the type of device. Note that this
    /// is a device-type-specific name; it has nothing to do with the name by which a user might
    /// have configured the device in a robot configuration.
    #[must_use]
    #[doc(alias = "getDeviceName")]
    pub fn get_device_name(&self) -> String {
        self.vm
            .attach_current_thread(|env| {
                let res = call_method!(
                    env env,
                    self.hardware_device,
                    "getDeviceName",
                    format!("()Ljava/lang/String;"),
                    []
                )
                .unwrap()
                .l()
                .unwrap();
                jni::errors::Result::Ok(
                    JString::cast_local(env, res)
                        .unwrap()
                        .mutf8_chars(env)
                        .unwrap()
                        .to_str()
                        .to_string(),
                )
            })
            .unwrap()
    }
    /// Get connection information about this device in a human readable format.
    #[must_use]
    #[doc(alias = "getConnectionInfo")]
    pub fn get_connection_info(&self) -> String {
        self.vm
            .attach_current_thread(|env| {
                let res = call_method!(
                    env env,
                    self.hardware_device,
                    "getConnectionInfo",
                    format!("()Ljava/lang/String;"),
                    []
                )
                .unwrap()
                .l()
                .unwrap();
                jni::errors::Result::Ok(
                    JString::cast_local(env, res)
                        .unwrap()
                        .mutf8_chars(env)
                        .unwrap()
                        .to_str()
                        .to_string(),
                )
            })
            .unwrap()
    }
    /// Resets the device's configuration to that which is expected at the beginning of an `OpMode`.
    /// For example, motors will reset the their direction to 'forward'.
    #[doc(alias = "resetDeviceConfigurationForOpMode")]
    pub fn reset_device_config(&self) {
        call_method!(void self, self.hardware_device, "resetDeviceConfigurationForOpMode", "()V", []);
    }
    /// Disables this device.
    #[doc(alias = "close")]
    pub fn disable(&self) {
        call_method!(void self, self.hardware_device, "close", "()V", []);
    }
}

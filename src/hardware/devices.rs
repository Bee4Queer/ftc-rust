//! Implementations of [`hardware::Device`](crate::hardware::Device).

use std::ops::RangeBounds;

use jni::{JavaVM, objects::JObject, refs::Global, signature::RuntimeMethodSignature};
use log::trace;

use crate::{debug_assert, debug_assert_ne};

use crate::{
    call_method, call_method_device, hardware::{
        AngularVelocity, Direction, IntoJniObject as _, Rev9AxisImuOrientationOnRobot, RunMode,
        YawPitchRollAngles, ZeroPowerBehavior, get_class,
    },
};

/// Easily define a basic device.
macro_rules! device {
    ($(#[$attr:meta])* $name:ident, JAVA_CLASS = $java_class:literal $(;)? $(,)? JNI_CLASS = $jni_class:literal $(;)? $(,)?) => {
        paste::paste! {
            $(#[$attr])*
            pub struct [< $name Inner >] {
                /// The environment.
                vm: JavaVM,
                /// The actual object.
                object: Global<JObject<'static>>,
            }

            impl std::fmt::Debug for [< $name Inner >] {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str(concat!("(opaque ", stringify!($name), " object, wraps ", $java_class, ")"))
                }
            }

            $(#[$attr])*
            /// 
            /// Default is essentially a null pointer and will panic upon attempted use.
            #[derive(Default)]
            #[repr(transparent)]
            pub struct $name {
                inner: Option<[< $name Inner >]>
            }

            impl $name {
                /// Returns whether this device is a null pointer.
                pub fn is_null(&self) -> bool {
                    self.inner.is_none()
                }
                #[allow(clippy::panic)]
                fn vm(&self) -> &JavaVM {
                    if let Some(inner) = self.inner.as_ref() {
                        &inner.vm
                    } else {
                        panic!("Attempted to use null device");
                    }
                }
                fn object(&self) -> &Global<JObject<'static>> {
                    if let Some(inner) = self.inner.as_ref() {
                        &inner.object
                    } else {
                        panic!("Attempted to use null device");
                    }
                }
            }

            impl std::fmt::Debug for $name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str(concat!("(opaque ", stringify!($name), " object, wraps ", $java_class, ")"))
                }
            }

            impl $crate::hardware::Device for $name {
                const JAVA_CLASS: &'static str = $java_class;
                const JNI_CLASS: &'static str = $jni_class;
                fn from_java(vm: JavaVM, object: Global<JObject<'static>>) -> Self {
                    Self {
                        inner: Some([< $name Inner >] {
                            vm,
                            object,
                        })
                    }
                }
            }
        }
    };
}

device!(
    /// Javadoc available at https://javadoc.io/doc/org.firstinspires.ftc/RobotCore/latest/com/qualcomm/robotcore/hardware/DcMotor.html.
    /// 
    /// `DcMotor` provides access to full-featured motor functionality.
    DcMotor,
    JAVA_CLASS = "com.qualcomm.robotcore.hardware.DcMotor";
    JNI_CLASS = "com/qualcomm/robotcore/hardware/DcMotor";
);

impl DcMotor {
    /// Sets the logical direction in which this motor operates.
    #[doc(alias = "setDirection")]
    pub fn set_direction(&self, dir: Direction) {
        trace!("setting direction of DcMotor");
        self.vm()
            .attach_current_thread(|env| {
                let obj = dir.into_jni_object_dcmotor(env);
                call_method!(
                    env env,
                    self.object(),
                    "setDirection",
                    format!("(L{};)V", Direction::DCMOTOR_JNI_CLASS),
                    [&obj]
                )?;
                jni::errors::Result::Ok(())
            })
            .unwrap();
        trace!("set direction of DcMotor");
    }

    /// Returns the current logical direction in which this motor is operating.
    #[doc(alias = "getDirection")]
    pub fn direction(&self) -> Direction {
        let res = call_method_device!(
            obj self,
            self.object(),
            "getDirection",
            format!("()L{};", Direction::DCMOTOR_JNI_CLASS),
            []
        );
        Direction::from_jni_object_dcmotor(self.vm(), res)
    }

    /// Sets the power level of the motor, expressed as a fraction of the maximum possible power /
    /// speed supported according to the run mode in which the motor is operating.
    ///
    /// See [`set_zero_power_behavior`](DcMotor::set_zero_power_behavior) for what happens when zero
    /// power is applied.
    ///
    /// In debug builds, setting this outside the range of -1.0..=1.0 will panic.
    #[doc(alias = "setPower")]
    pub fn set_power(&self, power: f64) {
        debug_assert!(
            (-1.0..=1.0).contains(&power),
            "motor power/speed should be contained within -1.0..=1.0"
        );

        call_method_device!(void self, self.object(), "setPower", "(D)V", [power]);
    }

    /// Returns the current configured power level of the motor.
    #[doc(alias = "getPower")]
    #[must_use]
    pub fn power(&self) -> f64 {
        call_method_device!(double self, self.object(), "getPower", "()D",
            [])
    }

    /// Sets the behavior of the motor when a power level of zero is applied.
    #[doc(alias = "setZeroPowerBehavior")]
    pub fn set_zero_power_behavior(&self, zpb: ZeroPowerBehavior) {
        debug_assert_ne!(zpb, ZeroPowerBehavior::Unknown);

        self.vm()
            .attach_current_thread(|env| {
                let obj = zpb.into_jni_object(env);
                call_method!(
                    env env,
                    self.object(),
                    "setZeroPowerBehavior",
                    format!("(L{};)V", ZeroPowerBehavior::JNI_CLASS),
                    [&obj]
                )?;
                jni::errors::Result::Ok(()) // cannot return a reference
            })
            .unwrap();
    }

    /// Returns the current behavior of the motor were a power level of zero to be applied.
    #[doc(alias = "getZeroPowerBehavior")]
    pub fn zero_power_behavior(&self) -> ZeroPowerBehavior {
        let res = call_method_device!(
            obj self,
            self.object(),
            "getZeroPowerBehavior",
            format!("()L{};", ZeroPowerBehavior::JNI_CLASS),
            []
        );
        ZeroPowerBehavior::from_jni_object(&self.vm(), res)
    }

    /// Sets the desired encoder target position to which the motor should advance or retreat and
    /// then actively hold there at. This behavior is similar to the operation of a servo. The
    /// maximum speed at which this advance or retreat occurs is governed by the power level
    /// currently set on the motor. While the motor is advancing or retreating to the desired
    /// target position, [`DcMotor::is_busy`] will return true.
    ///
    /// Note that adjustment to a target position is only effective when the motor is in
    /// [`RunMode::RunToPosition`]. Note further that, clearly, the motor must be equipped with
    /// an encoder in order for this mode to function properly.
    #[doc(alias = "setTargetPosition")]
    pub fn set_target_position(&self, target_pos: i32) {
        call_method_device!(void self, self.object(), "setTargetPosition", "(I)V", [target_pos]);
    }

    /// Returns the current target encoder position for this motor.
    #[doc(alias = "getTargetPosition")]
    #[must_use]
    pub fn target_position(&self) -> i32 {
        call_method_device!(int self, self.object(), "getTargetPosition", "()I",
            [])
    }

    /// Returns true if the motor is currently advancing or retreating to a target position.
    #[doc(alias = "isBusy")]
    #[must_use]
    pub fn busy(&self) -> bool {
        call_method_device!(bool self, self.object(), "isBusy", "()Z",
            [])
    }

    /// Returns the current reading of the encoder for this motor. The units for this reading,
    /// that is, the number of ticks per revolution, are specific to the motor/encoder in question,
    /// and thus are not specified here.
    #[doc(alias = "getCurrentPosition")]
    #[must_use]
    pub fn current_position(&self) -> i32 {
        call_method_device!(int self, self.object(), "getCurrentPosition", "()I",
            [])
    }

    /// Sets the behavior of the motor when a power level of zero is applied.
    #[doc(alias = "setMode")]
    pub fn set_mode(&self, mode: RunMode) {
        self.vm()
            .attach_current_thread(|env| {
                let obj = mode.into_jni_object(env);
                call_method!(
                    env env,
                    self.object(),
                    "setMode",
                    format!("(L{};)V", RunMode::JNI_CLASS),
                    [&obj]
                )?;
                jni::errors::Result::Ok(()) // cannot return a reference
            })
            .unwrap();
    }

    /// Returns the current behavior of the motor were a power level of zero to be applied.
    #[doc(alias = "getMode")]
    pub fn mode(&self) -> RunMode {
        let res = call_method_device!(
            obj self,
            self.object(),
            "getMode",
            format!("()L{};", RunMode::JNI_CLASS),
            []
        );
        RunMode::from_jni_object(&self.vm(), res)
    }
}

device!(
    /// Javadoc available at https://javadoc.io/doc/org.firstinspires.ftc/RobotCore/latest/com/qualcomm/robotcore/hardware/Servo.html.
    /// 
    /// `Servo` provides access to servo hardware devices.
    Servo,
    JAVA_CLASS = "com.qualcomm.robotcore.hardware.Servo";
    JNI_CLASS = "com/qualcomm/robotcore/hardware/Servo";
);

impl Servo {
    /// Sets the logical direction in which this servo operates.
    #[doc(alias = "setDirection")]
    pub fn set_direction(&self, dir: Direction) {
        self.vm()
            .attach_current_thread(|env| {
                let obj = dir.into_jni_object_servo(env);
                call_method!(
                    env env,
                    self.object(),
                    "setDirection",
                    format!("(L{};)V", Direction::SERVO_JNI_CLASS),
                    [&obj]
                )?;
                jni::errors::Result::Ok(()) // cannot return a reference
            })
            .unwrap();
    }

    /// Returns the current logical direction in which this servo is set as operating.
    #[doc(alias = "getDirection")]
    pub fn direction(&self) -> Direction {
        let res = call_method_device!(
            obj self,
            self.object(),
            "getDirection",
            format!("()L{};", Direction::SERVO_JNI_CLASS),
            []
        );
        Direction::from_jni_object_servo(&self.vm(), res)
    }

    /// Sets the current position of the servo, expressed as a fraction of its available range. If
    /// PWM power is enabled for the servo, the servo will attempt to move to the indicated
    /// position.
    #[doc(alias = "setPosition")]
    pub fn set_target_position(&self, target_pos: f64) {
        debug_assert!(
            (0.0..=1.0).contains(&target_pos),
            "servo target position must be within 0..=1"
        );
        call_method_device!(void self, self.object(), "setPosition", "(D)V", [target_pos]);
    }

    /// Returns the position to which the servo was last commanded to move. Note that this method
    /// does NOT read a position from the servo through any electrical means, as no such
    /// electrical mechanism is, generally, available.
    #[doc(alias = "getPosition")]
    #[must_use]
    pub fn target_position(&self) -> f64 {
        call_method_device!(double self, self.object(), "getPosition", "()D",
            [])
    }

    /// Scales the available movement range of the servo to be a subset of its maximum range.
    /// Subsequent positioning calls will operate within that subset range. This is useful if
    /// your servo has only a limited useful range of movement due to the physical hardware that
    /// it is manipulating (as is often the case) but you don't want to have to manually scale
    /// and adjust the input to [`Servo::set_target_position`] each time.
    ///
    /// For example, if `set_range(0.2..0.8)` is set then servo positions will be scaled to fit in
    /// that range. `set_target_position(0.0)` scales to 0.2, `set_target_position(1.0)` scales
    /// to 0.8 (notably exclusive bounds are essentially treated as inclusive here!),
    /// `set_target_position(0.5)` scales to 0.5, `set_target_position(0.25)` scales to 0.35, and
    /// `set_target_position(0.75)` scales to 0.65.
    ///
    /// Note the parameters passed here are relative to the underlying full range of motion of the
    /// servo, not its currently scaled range, if any. Thus, `set_range(0.0..1.0)` will reset
    /// the servo to its full range of movement. In Rust, `set_range(..)` will achieve the same effect.
    #[doc(alias = "scaleRange")]
    pub fn set_range(&self, range: impl RangeBounds<f64>) {
        let start = match range.start_bound().cloned() {
            std::ops::Bound::Included(v) => v,
            std::ops::Bound::Excluded(v) => v, // really terrible practice here
            std::ops::Bound::Unbounded => 0.0,
        };
        let end = match range.end_bound().cloned() {
            std::ops::Bound::Included(v) => v,
            std::ops::Bound::Excluded(v) => v, // really terrible practice here
            std::ops::Bound::Unbounded => 1.0,
        };
        debug_assert!(
            start >= 0.0 && end <= 1.0,
            "servo range should not be larger than 0.0, 1.0"
        );
        call_method_device!(void self, self.object(), "scaleRange", "(DD)V", [start, end]);
    }
}

device!(
    /// Javadoc available at https://javadoc.io/doc/org.firstinspires.ftc/RobotCore/latest/com/qualcomm/robotcore/hardware/CRServo.html.
    /// 
    /// `CRServo` is supported by continuous rotation servos
    CRServo,
    JAVA_CLASS = "com.qualcomm.robotcore.hardware.CRServo";
    JNI_CLASS = "com/qualcomm/robotcore/hardware/CRServo";
);

impl CRServo {
    /// Sets the logical direction in which this motor operates.
    #[doc(alias = "setDirection")]
    pub fn set_direction(&self, dir: Direction) {
        self.vm()
            .attach_current_thread(|env| {
                let obj = dir.into_jni_object_servo(env);
                call_method!(
                    env env,
                    self.object(),
                    "setDirection",
                    format!("(L{};)V", Direction::SERVO_JNI_CLASS),
                    [&obj]
                )?;
                jni::errors::Result::Ok(()) // cannot return a reference
            })
            .unwrap();
    }

    /// Returns the current logical direction in which this motor is set as operating.
    #[doc(alias = "getDirection")]
    pub fn direction(&self) -> Direction {
        let res = call_method_device!(
            obj self,
            self.object(),
            "getDirection",
            format!("()L{};", Direction::SERVO_JNI_CLASS),
            []
        );
        Direction::from_jni_object_servo(&self.vm(), res)
    }

    /// Sets the power level of the motor, expressed as a fraction of the maximum possible power /
    /// speed supported according to the run mode in which the motor is operating.
    ///
    /// Setting a power level of zero will brake the motor
    #[doc(alias = "setPower")]
    pub fn set_power(&self, power: f64) {
        debug_assert!(
            (-1.0..=1.0).contains(&power),
            "CRServo power/speed should be contained within -1.0..=1.0"
        );
        call_method_device!(void self, self.object(), "setPower", "(D)V", [power]);
    }

    /// Returns the current configured power level of the motor.
    #[doc(alias = "getPower")]
    #[must_use]
    pub fn power(&self) -> f64 {
        call_method_device!(double self, self.object(), "getPower", "()D",
            [])
    }
}

device!(
    /// Javadoc available at https://javadoc.io/doc/org.firstinspires.ftc/RobotCore/latest/com/qualcomm/robotcore/hardware/IMU.html.
    /// 
    /// An Inertial Measurement Unit that provides robot-centric orientation and angular velocity.
    ///
    /// All measurements are in the Robot Coordinate System. In the Robot Coordinate System, the X axis
    /// extends horizontally from your robot to the right, parallel to the ground. The Y axis extends
    /// horizontally from your robot straight ahead, parallel to the ground. The Z axis extends
    /// vertically from your robot, towards the ceiling.
    ///
    /// The Robot Coordinate System is right-handed, which means that if you point the thumb of a
    /// typical human right hand in the direction of an axis, rotation around that axis is defined as
    /// positive in the direction that the fingers curl.
    ///
    /// Orientation values are relative to the robot's position the last time that resetYaw was called,
    /// as if the robot was perfectly level at that time.
    ///
    /// The recommended way to read the orientation is as yaw, pitch, and roll angles, via
    /// getRobotYawPitchRollAngles. See the `YawPitchRollAngles` documentation for a full description of
    /// how the angles get applied to each other. That class's documentation will duplicate some
    /// information found here, but will not cover how yaw, pitch, and roll work in the specific context
    /// of an IMU that implements this interface.
    ///
    /// Yaw is the side-to-side lateral rotation of the robot. In terms of the Robot Coordinate System,
    /// it is defined as how far the robot has turned around the Z axis. Sometimes yaw is also referred
    /// to as "heading". The yaw can drift slowly over time, because most implementations do not use a
    /// magnetometer as an absolute reference (magnetometer readings are disrupted by nearby motors).
    /// The yaw reference is preserved between `OpMode` runs, unless the Robot Controller application is
    /// restarted, the Restart Robot option is selected, or the second `OpMode` calls resetYaw. This means
    /// that your yaw reference point can remain consistent through both the Autonomous and `TeleOp`
    /// phases of the match.
    ///
    /// Pitch is the front-to-back rotation of the robot. In terms of the Robot Coordinate System, it is
    /// how far the robot has turned around the X axis. Pitch uses gravity as an absolute reference, and
    /// will not drift over time.
    ///
    /// Roll is the side-to-side tilt of the robot. In terms of the Robot Coordinate System, it is
    /// defined as how far the robot has turned around the Y axis. Roll uses gravity as an absolute
    /// reference, and will not drift over time.
    ///
    /// All angles are in the range of -180 degrees to 180 degrees.
    ///
    /// The default orientation of the IMU on the robot will be implementation-specific. For the BNO055
    /// and BHI260AP implementations, the default is for a REV Control or Expansion Hub that is oriented
    /// with the USB ports facing the front of the robot and the REV logo facing the ceiling. To specify
    /// a non-default orientation on the robot, you need to call initialize.
    IMU,
    JAVA_CLASS = "com.qualcomm.robotcore.hardware.CRServo";
    JNI_CLASS = "com/qualcomm/robotcore/hardware/CRServo";
);

impl IMU {
    /// Resets the robot's yaw angle to 0. After calling this method, the reported orientation will
    /// be relative to the robot's position when this method was called, as if the robot was
    /// perfectly level right then. That is to say, the pitch and yaw will be ignored when this
    /// method is called.
    ///
    /// Unlike yaw, pitch and roll are always relative to gravity, and never need to be reset.
    #[doc(alias = "resetYaw")]
    pub fn reset_yaw(&self) {
        call_method_device!(void self, self.object(), "resetYaw", "()V", []);
    }
    /// Get the [`YawPitchRollAngles`] of the robot.
    #[doc(alias = "getRobotYawPitchRollAngles")]
    pub fn angles(&self) -> YawPitchRollAngles {
        let res = call_method_device!(
            obj self,
            self.object(),
            "getRobotYawPitchRollAngles",
            format!("()L{};", YawPitchRollAngles::JNI_CLASS),
            []
        );
        YawPitchRollAngles::from_jni_object(&self.vm(), res)
    }
    /// Initializes the IMU with non-default settings.
    #[doc(alias = "initialize")]
    #[must_use]
    pub fn init(&self, orientation: Rev9AxisImuOrientationOnRobot) -> bool {
        self.vm()
            .attach_current_thread(|env| {
                let orientation = orientation.into_jni_object(env);
                let class = get_class(env, "com/qualcomm/robotcore/hardware/IMU$Parameters");

                let params = env
                    .new_object(
                        class,
                        RuntimeMethodSignature::from_str(format!(
                            "(L{};)Lcom/qualcomm/robotcore/hardware/IMU$Parameters;",
                            Rev9AxisImuOrientationOnRobot::JNI_CLASS
                        ))?
                        .method_signature(),
                        &[(&orientation).into()],
                    )?;

                call_method!(
                    env env,
                    self.object(),
                    "initialize",
                    "(Lcom/qualcomm/robotcore/hardware/IMU$Parameters;)Z",
                    [&params]
                )?
                .z()
            })
            .unwrap()
    }
    /// Get the [`AngularVelocity`] of the robot.
    #[doc(alias = "getRobotAngularVelocity")]
    pub fn velocity(&self) -> AngularVelocity {
        let res = call_method_device!(
            obj self,
            self.object(),
            "getRobotAngularVelocity",
            format!("()L{};", AngularVelocity::JNI_CLASS),
            []
        );
        AngularVelocity::from_jni_object(&self.vm(), res)
    }
}

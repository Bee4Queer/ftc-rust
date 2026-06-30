//! Code for using Rust in FTC robot code.

use std::{
    any::Any,
    collections::HashMap,
    fmt::Debug,
    sync::{Arc, LazyLock, Mutex, atomic::AtomicI64},
    time::Duration,
};

#[cfg(feature = "proc-macro")]
pub use ftc_rust_proc::ftc;
pub use jni;
use jni::{jni_sig, jni_str, objects::JObject, refs::Global, strings::JNIString, vm::JavaVM};
pub use log;
use log::{trace, warn};

use crate::{
    command::{Command, SCHEDULER},
    hardware::Hardware,
};

pub mod command;
pub mod hardware;

#[macro_use]
mod macros;

/// A wrapper for accessing telemetry-related methods.
#[must_use]
pub struct Telemetry {
    /// The environment.
    vm: JavaVM,
    /// The actual telemetry object. Should be org/firstinspires/ftc/robotcore/external/Telemetry.
    telemetry: Global<JObject<'static>>,
}

impl Debug for Telemetry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("(opaque Telemetry object)")
    }
}

impl Telemetry {
    /// Adds an item to the end if the telemetry being built for driver station display. The caption
    /// and value are shown on the driver station separated by the caption value separator. The
    /// item is removed if `clear` or `clear_all` is called.
    #[allow(clippy::needless_pass_by_value)]
    pub fn add_data(&self, caption: impl ToString, value: impl ToString) {
        self.vm
            .attach_current_thread(|env| {
                let caption = new_string!(env env, caption.to_string())?;
                let value = new_string!(env env, value.to_string())?;
                call_method!(
                    env env,
                    self.telemetry,
                    "addData",
                    "(Ljava/lang/String;Ljava/lang/Object;)Lorg/firstinspires/ftc/robotcore/external/Telemetry$Item;",
                    [&caption, &value]
                )
                ?;
                jni::errors::Result::Ok(()) // rust wants to know what the return type is
            })
            .unwrap();
    }
    /// Sends the receiver `Telemetry` to the driver station if more than the transmission interval
    /// has elapsed since the last transmission, or schedules the transmission of the receiver
    /// should no subsequent `Telemetry` state be scheduled for transmission before the
    /// transmission interval expires.
    pub fn update(&self) {
        call_method!(
            void self,
            self.telemetry,
            "update",
            "()Z",
            []
        );
    }
    /// Removes all items from the receiver whose value is not to be retained.
    pub fn clear(&self) {
        call_method!(
            void self,
            self.telemetry,
            "clear",
            "()V",
            []
        );
    }
    /// Removes all items, lines, and actions from the receiver.
    pub fn clear_all(&self) {
        call_method!(
            void self,
            self.telemetry,
            "clearAll",
            "()V",
            []
        );
    }
}

/// A gamepad.
#[must_use]
pub struct Gamepad {
    /// The java environment.
    vm: JavaVM,
    /// The gamepad object. Should be a com/qualcomm/robotcore/hardware/Gamepad.
    #[allow(clippy::struct_field_names)]
    gamepad: Global<JObject<'static>>,
    /// The gamepad being used.
    which: WhichGamepad,
}

impl Debug for Gamepad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("(opaque Gamepad object)")
    }
}

/// `snake_case` to `camelCase`, used mostly in macro expansions for Gamepad
fn snake_to_camel(s: &str) -> String {
    let mut first_done = false;
    s.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            if !first_done {
                first_done = true;
                return part.to_string();
            }
            let mut out = String::with_capacity(part.len());
            let mut chars = part.chars();
            out.push_str(&chars.next().unwrap().to_uppercase().collect::<String>());
            out.push_str(chars.as_str());
            out
        })
        .collect()
}

/// A controller button.
#[allow(missing_docs, reason = "idgaf")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Button {
    A,
    B,
    X,
    Y,
    Circle,
    Cross,
    Triangle,
    Square,

    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,

    Guide,
    Start,
    Back,
    Share,
    Options,

    LeftBumper,
    RightBumper,

    LeftStick,
    RightStick,

    Touchpad,
    TouchpadFinger1,
    TouchpadFinger2,

    Ps,

    LeftTrigger,
    RightTrigger,
}

#[allow(missing_docs, reason = "idgaf")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stick {
    LeftStickX,
    LeftStickY,

    RightStickX,
    RightStickY,

    LeftTrigger,
    RightTrigger,

    TouchpadFinger1X,
    TouchpadFinger1Y,

    TouchpadFinger2X,
    TouchpadFinger2Y,
}

#[allow(missing_docs, reason = "idgaf")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WhichGamepad {
    Gamepad1,
    Gamepad2,
}

#[allow(missing_docs, reason = "idgaf")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PressEdge {
    Press,
    Release,
    WhilePressed,
    WhileReleased,
}

/// The command used for button presses. Registered by the `Gamepad::on_*` and `Gamepad::while_`
/// functions.
#[derive(Debug)]
pub struct ButtonCommand<F: FnMut(PressEdge) + 'static + Send + Sync> {
    /// The gamepad to check.
    pub gamepad: WhichGamepad,
    /// The button to check.
    pub button: Button,
    /// The edge to check ([`Gamepad::was_pressed`], [`Gamepad::was_released`],
    /// [`Gamepad::is_pressed`], or [`Gamepad::is_released`])
    pub edge: PressEdge,
    /// The function to call when the condition is met.
    pub f: F,
}

impl<F: FnMut(PressEdge) + 'static + Send + Sync> Command for ButtonCommand<F> {
    fn execute(&mut self, _: &FtcContext) {
        (self.f)(self.edge);
    }
    fn try_run(&mut self, ctx: &FtcContext) -> bool {
        let gamepad = match self.gamepad {
            WhichGamepad::Gamepad1 => ctx.gamepad1(),
            WhichGamepad::Gamepad2 => ctx.gamepad2(),
        };

        match self.edge {
            PressEdge::WhilePressed => gamepad.is_pressed(self.button),
            PressEdge::WhileReleased => gamepad.is_released(self.button),
            PressEdge::Press => gamepad.was_pressed(self.button),
            PressEdge::Release => gamepad.was_released(self.button),
        }
    }
}

/// The command used for stick thresholds.
#[derive(Debug)]
pub struct StickCommand<F: FnMut(f32) + 'static + Send + Sync> {
    /// The gamepad to check.
    pub gamepad: WhichGamepad,
    /// The stick to check.
    pub stick: Stick,
    /// The threshold it must meet to activate. If negative, it must be less than this, if positive
    /// it must be greater.
    pub threshold: f32,
    /// If false, the absolute value is taken of the stick first.
    pub abs: bool,
    /// The function to call when the condition is met.
    pub f: F,
}

impl<F: FnMut(f32) + 'static + Send + Sync> Command for StickCommand<F> {
    fn execute(&mut self, ctx: &FtcContext) {
        let gamepad = match self.gamepad {
            WhichGamepad::Gamepad1 => ctx.gamepad1(),
            WhichGamepad::Gamepad2 => ctx.gamepad2(),
        };

        let value = gamepad.get_stick(self.stick);

        (self.f)(value);
    }
    fn try_run(&mut self, ctx: &FtcContext) -> bool {
        let gamepad = match self.gamepad {
            WhichGamepad::Gamepad1 => ctx.gamepad1(),
            WhichGamepad::Gamepad2 => ctx.gamepad2(),
        };
        let value = gamepad.get_stick(self.stick);
        let value = if self.abs { value } else { value.abs() };
        let threshold = if self.abs {
            self.threshold
        } else {
            self.threshold.abs()
        };

        if threshold < 0.0 {
            value < threshold
        } else {
            value > threshold
        }
    }
}

/// A macro for gamepad inputs.
macro_rules! gamepad_button {
    ($($(#[$attr:meta])* $vis:vis button $name:ident $ty_name:ident)*) => {
        paste::paste! {$($(#[$attr])*
            #[must_use]
        $vis fn $name (&self) -> bool {
            self
                .vm
                .attach_current_thread(|env| {
                    env.get_field(
                        &self.gamepad,
                        JNIString::new(stringify!($name)),
                        jni_sig!("Z"),
                    )
                    ?
                    .z()
                })
                .unwrap()
        }

        #[doc = concat!("Checks if ", stringify!($name), " was pressed since the last call of this method")]
        #[must_use]
        $vis fn [< $name _was_pressed >] (&self) -> bool {
            call_method!(bool self, self.gamepad, snake_to_camel(stringify!($name)) + "WasPressed", "()Z", [])
        }

        #[doc = concat!("Checks if ", stringify!($name), " was released since the last call of this method")]
        #[must_use]
        $vis fn [< $name _was_released >] (&self) -> bool {
            call_method!(bool self, self.gamepad, snake_to_camel(stringify!($name)) + "WasReleased", "()Z", [])
        }

        /// Execute the provided function when the provided edge occurs.
        $vis fn [< execute_on_ $name >] (
            &self,
            f: impl FnMut(PressEdge) + 'static + Send + Sync,
            edge: PressEdge
        ) {
            self.execute_on(Button:: $ty_name, f, edge);
        }

        #[doc = concat!("Runs the provided function whenever ", stringify!($name), " is pressed.")]
        $vis fn [< on_press_ $name >] (&self, f: impl FnMut(PressEdge) + 'static + Send + Sync) {
            self.[< execute_on_ $name >] (f, PressEdge::Press);
        }

        #[doc = concat!("Runs the provided function whenever ", stringify!($name), " is released.")]
        $vis fn [< on_release_ $name >] (&self, f: impl FnMut(PressEdge) + 'static + Send + Sync) {
            self.[< execute_on_ $name >] (f, PressEdge::Release);
        }

        #[doc = concat!("Runs the provided function while ", stringify!($name), " is pressed.")]
        $vis fn [< while_press_ $name >] (&self, f: impl FnMut(PressEdge) + 'static + Send + Sync) {
            self.[< execute_on_ $name >] (f, PressEdge::WhilePressed);
        }

        #[doc = concat!("Runs the provided function while ", stringify!($name), " is released.")]
        $vis fn [< while_release_ $name >] (&self, f: impl FnMut(PressEdge) + 'static + Send + Sync) {
            self.[< execute_on_ $name >] (f, PressEdge::WhileReleased);
        })*
        /// Return whether the specified button is pressed.
        #[must_use]
        pub fn is_pressed(&self, button: Button) -> bool {
            match button {
                $(Button:: $ty_name => self. $name (), )*
                Button::LeftTrigger => self.left_trigger_pressed(),
                Button::RightTrigger => self.right_trigger_pressed(),
            }
        }
        /// Return whether the specified button was newly pressed since the last call.
        #[must_use]
        pub fn was_pressed(&self, button: Button) -> bool {
            match button {
                $(Button:: $ty_name => self. [< $name _was_pressed >] (), )*
                Button::LeftTrigger => self.left_trigger_was_pressed(),
                Button::RightTrigger => self.right_trigger_was_pressed(),
            }
        }
        /// Return whether the specified button was newly released since the last call.
        #[must_use]
        pub fn was_released(&self, button: Button) -> bool {
            match button {
                $(Button:: $ty_name => self. [< $name _was_released >] (), )*
                Button::LeftTrigger => self.left_trigger_was_released(),
                Button::RightTrigger => self.right_trigger_was_released(),
            }
        } }
    };
    ($($(#[$attr:meta])* $vis:vis float $name:ident $ty_name:ident)*) => {
        paste::paste! {
            $($(#[$attr])*
            $vis fn $name (&self) -> f32 {
                self
                    .vm
                    .attach_current_thread(|env| {
                        env.get_field(
                            &self.gamepad,
                            JNIString::new(stringify!($name)),
                            jni_sig!("F"),
                        )
                        ?
                        .f()
                    })
                    .unwrap()
            }

            /// Call the provided function when the threshold is passed. Called repeatedly. If
            /// `dir` is true, then the direction matters.
            $vis fn [< on_ $name >] (&self,
                f: impl FnMut(f32) + 'static + Send + Sync,
                threshold: f32,
                dir: bool
            ) {
                self.execute_on_stick(Stick:: $ty_name, threshold, dir, f);
            } )*
            /// Get the value of the provided stick.
            pub fn get_stick(&self, stick: Stick) -> f32 {
                match stick {
                    $(Stick:: $ty_name => self. $name ()),*
                }
            }
        }
    };
}

impl Gamepad {
    /// Clears any remembered presses and releases of buttons.
    #[doc(alias = "resetEdgeDetection")]
    pub fn reset_edge_detection(&self) {
        call_method!(void self, self.gamepad, "resetEdgeDetection", "()V", []);
    }
    /// Set the threshold for determining if a trigger is pressed.
    #[doc(alias = "setTriggerThreshold")]
    pub fn set_trigger_threshold(&self, thresh: f32) {
        call_method!(void self, self.gamepad, "setTriggerThreshold", "(F)V", [thresh]);
    }
    /// Get the threshold for determining if a trigger is pressed.
    #[doc(alias = "getTriggerThreshold")]
    #[must_use]
    pub fn get_trigger_threshold(&self) -> f32 {
        call_method!(float self, self.gamepad, "getTriggerThreshold", "()F", [])
    }
    /// Return whether the specified button is released.
    #[must_use]
    pub fn is_released(&self, button: Button) -> bool {
        !self.is_pressed(button)
    }
    /// Execute the provided function when the provided edge of the provided button occurs.
    pub fn execute_on(
        &self,
        button: Button,
        f: impl FnMut(PressEdge) + 'static + Send + Sync,
        edge: PressEdge,
    ) {
        (ButtonCommand {
            gamepad: self.which,
            button,
            f,
            edge,
        })
        .schedule();
    }
    /// Execute the provided function when the provided edge of the provided stick occurs.
    pub fn execute_on_stick(
        &self,
        stick: Stick,
        threshold: f32,
        dir: bool,
        f: impl FnMut(f32) + 'static + Send + Sync,
    ) {
        (StickCommand {
            gamepad: self.which,
            stick,
            f,
            threshold,
            abs: dir,
        })
        .schedule();
    }
    gamepad_button!(
        /// The A button.
        pub button a A
        /// The B button.
        pub button b B
        /// The X button.
        pub button x X
        /// The Y button.
        pub button y Y

        /// The circle button.
        pub button circle Circle
        /// The cross button.
        pub button cross Cross
        /// The triangle button.
        pub button triangle Triangle
        /// The square button.
        pub button square Square

        /// The up arrow on the dpad.
        pub button dpad_up DpadUp
        /// The down arrow on the dpad.
        pub button dpad_down DpadDown
        /// The left arrow on the dpad.
        pub button dpad_left DpadLeft
        /// The right arrow on the dpad.
        pub button dpad_right DpadRight

        /// The guide button.
        pub button guide Guide
        /// The start button.
        pub button start Start
        /// The back button.
        pub button back Back
        /// The share button.
        pub button share Share
        /// The options button.
        pub button options Options

        /// The left bumper.
        pub button left_bumper LeftBumper
        /// The right bumper.
        pub button right_bumper RightBumper

        /// The left stick button.
        pub button left_stick_button LeftStick
        /// The right stick button.
        pub button right_stick_button RightStick

        /// The touchpad.
        pub button touchpad Touchpad
        /// The first finger on the touchpad.
        pub button touchpad_finger_1 TouchpadFinger1
        /// The second finger on the touchpad.
        pub button touchpad_finger_2 TouchpadFinger2

        /// No idea what this is.
        pub button ps Ps
    );

    /// Boolean value of if the left trigger is past `DEFAULT_TRIGGER_THRESHOLD`.
    #[must_use]
    pub fn left_trigger_pressed(&self) -> bool {
        self.vm
            .attach_current_thread(|env| {
                env.get_field(
                    &self.gamepad,
                    JNIString::new("left_trigger_pressed"),
                    jni_sig!("Z"),
                )?
                .z()
            })
            .unwrap()
    }
    ///Checks if `left_trigger` was pressed since the last call of this method
    #[must_use]
    pub fn left_trigger_was_pressed(&self) -> bool {
        {
            self.vm
                .attach_current_thread(|env| {
                    {
                        let env: &mut crate::jni::Env = env;
                        let obj = env.new_local_ref(&self.gamepad)?;
                        env.call_method(
                            &obj,
                            crate::jni::strings::JNIString::new("leftTriggerWasPressed"),
                            crate::jni::signature::RuntimeMethodSignature::from_str("()Z")?
                                .method_signature(),
                            &[],
                        )
                    }?
                    .z()
                })
                .unwrap()
        }
    }
    ///Checks if`left_trigger`was released since the last call of this method
    #[must_use]
    pub fn left_trigger_was_released(&self) -> bool {
        {
            self.vm
                .attach_current_thread(|env| {
                    {
                        let env: &mut crate::jni::Env = env;
                        let obj = env.new_local_ref(&self.gamepad)?;
                        env.call_method(
                            &obj,
                            crate::jni::strings::JNIString::new("leftTriggerWasReleased"),
                            crate::jni::signature::RuntimeMethodSignature::from_str("()Z")?
                                .method_signature(),
                            &[],
                        )
                    }?
                    .z()
                })
                .unwrap()
        }
    }
    /// Execute the provided function when the provided edge occurs.
    pub fn execute_on_left_trigger_pressed(
        &self,
        f: impl FnMut(PressEdge) + 'static + Send + Sync,
        edge: PressEdge,
    ) {
        self.execute_on(Button::LeftTrigger, f, edge);
    }
    ///Runs the provided function whenever `left_trigger` is pressed.
    pub fn on_press_left_trigger(&self, f: impl FnMut(PressEdge) + 'static + Send + Sync) {
        self.execute_on_left_trigger_pressed(f, PressEdge::Press);
    }
    ///Runs the provided function whenever `left_trigger` is released.
    pub fn on_release_left_trigger(&self, f: impl FnMut(PressEdge) + 'static + Send + Sync) {
        self.execute_on_left_trigger_pressed(f, PressEdge::Release);
    }
    ///Runs the provided function while `left_trigger` is pressed.
    pub fn while_press_left_trigger(&self, f: impl FnMut(PressEdge) + 'static + Send + Sync) {
        self.execute_on_left_trigger_pressed(f, PressEdge::WhilePressed);
    }
    ///Runs the provided function while `left_trigger` is released.
    pub fn while_release_left_trigger(&self, f: impl FnMut(PressEdge) + 'static + Send + Sync) {
        self.execute_on_left_trigger_pressed(f, PressEdge::WhileReleased);
    }
    /// Boolean value of if the right trigger is past `DEFAULT_TRIGGER_THRESHOLD`.
    #[must_use]
    pub fn right_trigger_pressed(&self) -> bool {
        self.vm
            .attach_current_thread(|env| {
                env.get_field(
                    &self.gamepad,
                    JNIString::new("right_trigger_pressed"),
                    jni_sig!("Z"),
                )?
                .z()
            })
            .unwrap()
    }
    ///Checks if `right_trigger` was pressed since the last call of this method
    #[must_use]
    pub fn right_trigger_was_pressed(&self) -> bool {
        {
            self.vm
                .attach_current_thread(|env| {
                    {
                        let env: &mut crate::jni::Env = env;
                        let obj = env.new_local_ref(&self.gamepad)?;
                        env.call_method(
                            &obj,
                            crate::jni::strings::JNIString::new("rightTriggerWasPressed"),
                            crate::jni::signature::RuntimeMethodSignature::from_str("()Z")?
                                .method_signature(),
                            &[],
                        )
                    }?
                    .z()
                })
                .unwrap()
        }
    }
    ///Checks if `right_trigger` was released since the last call of this method
    #[must_use]
    pub fn right_trigger_was_released(&self) -> bool {
        {
            self.vm
                .attach_current_thread(|env| {
                    {
                        let env: &mut crate::jni::Env = env;
                        let obj = env.new_local_ref(&self.gamepad)?;
                        env.call_method(
                            &obj,
                            crate::jni::strings::JNIString::new("lightTriggerWasReleased"),
                            crate::jni::signature::RuntimeMethodSignature::from_str("()Z")?
                                .method_signature(),
                            &[],
                        )
                    }?
                    .z()
                })
                .unwrap()
        }
    }
    /// Execute the provided function when the provided edge occurs.
    pub fn execute_on_right_trigger_pressed(
        &self,
        f: impl FnMut(PressEdge) + 'static + Send + Sync,
        edge: PressEdge,
    ) {
        self.execute_on(Button::RightTrigger, f, edge);
    }
    ///Runs the provided function whenever `right_trigger` is pressed.
    pub fn on_press_right_trigger(&self, f: impl FnMut(PressEdge) + 'static + Send + Sync) {
        self.execute_on_right_trigger_pressed(f, PressEdge::Press);
    }
    ///Runs the provided function whenever `right_trigger` is released.
    pub fn on_release_right_trigger(&self, f: impl FnMut(PressEdge) + 'static + Send + Sync) {
        self.execute_on_right_trigger_pressed(f, PressEdge::Release);
    }
    ///Runs the provided function while `right_trigger` is pressed.
    pub fn while_press_right_trigger(&self, f: impl FnMut(PressEdge) + 'static + Send + Sync) {
        self.execute_on_right_trigger_pressed(f, PressEdge::WhilePressed);
    }
    ///Runs the provided function while `right_trigger` is released.
    pub fn while_release_right_trigger(&self, f: impl FnMut(PressEdge) + 'static + Send + Sync) {
        self.execute_on_right_trigger_pressed(f, PressEdge::WhileReleased);
    }

    gamepad_button!(
        /// The X coordinate of the left stick.
        pub float left_stick_x LeftStickX
        /// The Y coordinate of the left stick.
        pub float left_stick_y LeftStickY

        /// The X coordinate of the right stick.
        pub float right_stick_x RightStickX
        /// The Y coordinate of the right stick.
        pub float right_stick_y RightStickY

        /// The left trigger.
        pub float left_trigger LeftTrigger
        /// The right trigger.
        pub float right_trigger RightTrigger

        /// The X coordinate of the first finger on the touchpad.
        pub float touchpad_finger_1_x TouchpadFinger1X
        /// The Y coordinate of the first finger on the touchpad.
        pub float touchpad_finger_1_y TouchpadFinger1Y

        /// The X coordinate of the second finger on the touchpad.
        pub float touchpad_finger_2_x TouchpadFinger2X
        /// The Y coordinate of the second finger on the touchpad.
        pub float touchpad_finger_2_y TouchpadFinger2Y
    );
}

/// A context used for accessing the Java runtime. Note that cloning is somewhat costly from
/// creating a new JNI global reference to the `this` object, so prefer passing around references
/// rather than owned contexts.
pub struct FtcContext {
    /// The java environment.
    vm: JavaVM,
    /// The op mode class.
    this: Global<JObject<'static>>,
}

/// User state
static STATE: LazyLock<Mutex<HashMap<i64, Box<dyn Any + Send + Sync + 'static>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static OPMODE_COUNTER: AtomicI64 = AtomicI64::new(1);

impl Debug for FtcContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("(opaque FtcContext object)")
    }
}

impl Clone for FtcContext {
    fn clone(&self) -> Self {
        Self {
            this: self
                .vm
                .attach_current_thread(|env| {
                    let local_ref = env.new_local_ref(&self.this)?;
                    env.new_global_ref(local_ref)
                })
                .unwrap(),
            vm: self.vm.clone(),
        }
    }
}

impl FtcContext {
    /// Create a new context.
    #[doc(hidden)]
    #[must_use]
    pub fn new<'local>(env: &mut jni::Env<'local>, this: JObject<'local>) -> Self {
        android_logger::init_once(android_logger::Config::default().with_max_level(
            if cfg!(debug_assertions) {
                log::LevelFilter::Trace
            } else {
                log::LevelFilter::Warn
            },
        ));

        trace!("Rust FTC initalized");

        Self::new_no_log(env, this)
    }
    /// Create a new context.
    #[doc(hidden)]
    #[must_use]
    pub fn new_no_log<'local>(env: &mut jni::Env<'local>, this: JObject<'local>) -> Self {
        let out = Self {
            this: env.new_global_ref(this).unwrap(),
            vm: env.get_java_vm().unwrap(),
        };
        if out.get_id() == 0 {
            out.vm
                .attach_current_thread(|env| {
                    let local_ref = env.new_local_ref(&out.this)?;
                    env.set_field(
                        local_ref,
                        jni_str!("rust_id"),
                        jni_sig!("J"),
                        OPMODE_COUNTER
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                            .into(),
                    )?;
                    jni::errors::Result::Ok(())
                })
                .unwrap();
        }
        out
    }
    /// Call a method with the state of this context. Panics if the associated state isn't the provided type.
    pub fn with_state<State: Any + Default + Send + Sync + 'static, R>(
        &self,
        f: impl FnOnce(&mut State) -> R,
    ) -> R {
        let mut lock = STATE.lock().unwrap();
        let state = lock
            .entry(self.get_id())
            .or_insert_with(|| Box::new(State::default()))
            .downcast_mut::<State>()
            .unwrap();
        f(state)
    }
    /// Get the unique ID for this opmode.
    pub fn get_id(&self) -> i64 {
        self.vm
            .attach_current_thread(|env| {
                let local_ref = env.new_local_ref(&self.this)?;
                jni::errors::Result::Ok(
                    env.get_field(local_ref, jni_str!("rust_id"), jni_sig!("J"))?
                        .into_long()
                        .unwrap(),
                )
            })
            .unwrap()
    }
    /// Whether the currently running opmode is iterative.
    #[must_use]
    pub fn is_iterative(&self) -> bool {
        ITERATIVE_CONTEXTS
            .lock()
            .unwrap()
            .contains_key(&self.get_id())
    }
    /// Whether the currently running opmode is linear.
    #[must_use]
    pub fn is_linear(&self) -> bool {
        !self.is_iterative()
    }
    /// The current stage of the opmode.
    #[must_use]
    pub fn current_stage(&self) -> OpModeStage {
        if self.is_iterative() {
            ITERATIVE_CONTEXTS
                .lock()
                .unwrap()
                .get(&self.get_id())
                .unwrap()
                .inner
                .lock()
                .unwrap()
                .stage
        } else if self.running() {
            OpModeStage::Running
        } else if call_method!(bool self, self.this, "opModeInInit", "()Z", []) {
            OpModeStage::Init
        } else {
            OpModeStage::Stop
        }
    }
    /// Return the telemetry object.
    pub fn telemetry(&self) -> Telemetry {
        trace!("Retrieved telemetry");

        let telemetry = self
            .vm
            .attach_current_thread(|env| {
                new_global!(
                    env,
                    env.get_field(
                        &self.this,
                        JNIString::new("telemetry"),
                        jni_sig!("Lorg/firstinspires/ftc/robotcore/external/Telemetry;"),
                    )?
                    .l()?
                )
            })
            .unwrap();

        Telemetry {
            vm: self.vm.clone(),
            telemetry,
        }
    }
    /// Return the hardware object.
    pub fn hardware(&self) -> Hardware {
        trace!("Retrieved hardware");

        let hardware_map = self
            .vm
            .attach_current_thread(|env| {
                new_global!(
                    env,
                    env.get_field(
                        &self.this,
                        JNIString::new("hardwareMap"),
                        jni_sig!("Lcom/qualcomm/robotcore/hardware/HardwareMap;"),
                    )?
                    .l()?
                )
            })
            .unwrap();

        Hardware {
            vm: self.vm.clone(),
            hardware_map,
        }
    }
    /// Return the first gamepad.
    pub fn gamepad1(&self) -> Gamepad {
        let gamepad = self
            .vm
            .attach_current_thread(|env| {
                new_global!(
                    env,
                    env.get_field(
                        &self.this,
                        JNIString::new("gamepad1"),
                        jni_sig!("Lcom/qualcomm/robotcore/hardware/Gamepad;"),
                    )?
                    .l()?
                )
            })
            .unwrap();

        Gamepad {
            which: WhichGamepad::Gamepad1,
            vm: self.vm.clone(),
            gamepad,
        }
    }
    /// Return the second gamepad.
    pub fn gamepad2(&self) -> Gamepad {
        let gamepad = self
            .vm
            .attach_current_thread(|env| {
                new_global!(
                    env,
                    env.get_field(
                        &self.this,
                        JNIString::new("gamepad2"),
                        jni_sig!("Lcom/qualcomm/robotcore/hardware/Gamepad;"),
                    )?
                    .l()?
                )
            })
            .unwrap();

        Gamepad {
            which: WhichGamepad::Gamepad2,
            vm: self.vm.clone(),
            gamepad,
        }
    }
    /// Wait for the driver to press play.
    #[doc(alias = "waitForStart")]
    pub fn wait_for_start(&self) {
        if !self.is_linear() {
            warn!("wait_for_start only exists in linear op modes; returning immediately");
            return;
        }
        call_method!(void self, self.this, "waitForStart", "()V", []);
    }
    /// Run the scheduler.
    #[doc(hidden)]
    pub fn run_scheduler(&self) {
        SCHEDULER.write().unwrap().run(self.clone());
    }
    /// Returns whether the `OpMode` is still running. If not (and a linear opmode), the op mode should exit as fast as
    /// possible.
    #[doc(alias = "opModeIsActive")]
    #[must_use]
    pub fn running(&self) -> bool {
        if self.is_iterative() {
            return self.current_stage() == OpModeStage::Running;
        }
        call_method!(bool self, self.this, "opModeIsActive", "()Z", [])
    }
    /// Get the amount of time the opmode has been running for.
    #[doc(alias = "getRuntime")]
    #[must_use]
    pub fn runtime(&self) -> Duration {
        let secs = call_method!(float self, self.this, "getRuntime", "()F", []);
        Duration::from_secs_f32(secs)
    }
    /// Reset the opmode's runtime.
    #[doc(alias = "resetRuntime")]
    pub fn reset_runtime(&self) {
        call_method!(void self, self.this, "resetRuntime", "()V", []);
    }
    /// Terminate the opmode NOW. Does not wait for anything. This function does not return.
    #[doc(alias = "terminateOpModeNow")]
    pub fn terminate_opmode(&self) -> ! {
        call_method!(void self, self.this, "terminateOpModeNow", "()V", []);
        unreachable!("terminateOpModeNow did not diverge");
    }
    /// Terminate the opmode, as if the driver had pressed the stop button on the controller.
    #[doc(alias = "requestOpModeStop")]
    pub fn request_stop(&self) {
        call_method!(void self, self.this, "requestOpModeStop", "()V", []);
    }
}

/// The current stage of an iterative opmode.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum OpModeStage {
    #[default]
    Init,
    Running,
    Stop,
}

/// The actual contexts. Used by [`IterativeContext::get_for`].
static ITERATIVE_CONTEXTS: LazyLock<Mutex<HashMap<i64, IterativeContext>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The actual data for [`IterativeContext`]
#[derive(Default)]
#[allow(clippy::type_complexity)]
struct InnerIterativeContext {
    /// `init` callbacks
    init: Vec<Box<dyn FnMut(&FtcContext) + Send + 'static>>,
    /// `init_loop` callbacks
    init_loop: Vec<Box<dyn FnMut(&FtcContext) + Send + 'static>>,
    /// `start` callbacks
    start: Vec<Box<dyn FnMut(&FtcContext) + Send + 'static>>,
    /// `r#loop` callbacks
    r#loop: Vec<Box<dyn FnMut(&FtcContext) + Send + 'static>>,
    /// `stop` callbacks
    stop: Vec<Box<dyn FnMut(&FtcContext) + Send + 'static>>,
    /// The current stage of this opmode.
    stage: OpModeStage,
}

/// Type used to register callbacks for an iterative op mode.
#[derive(Clone)]
pub struct IterativeContext {
    /// The reference-counted actual data.
    inner: Arc<Mutex<InnerIterativeContext>>,
}

impl Debug for IterativeContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("(opaque IterativeContext object)")
    }
}

impl IterativeContext {
    #[doc(hidden)]
    pub fn get_for<'local>(env: &mut jni::Env<'local>, this: &JObject<'local>) -> Self {
        let this = env.new_local_ref(this).unwrap();

        let id = FtcContext::new_no_log(env, this).get_id();
        match ITERATIVE_CONTEXTS.lock().unwrap().entry(id) {
            std::collections::hash_map::Entry::Occupied(occupied_entry) => {
                occupied_entry.get().clone()
            }
            std::collections::hash_map::Entry::Vacant(vacant_entry) => vacant_entry
                .insert(IterativeContext {
                    inner: Arc::new(Mutex::new(InnerIterativeContext::default())),
                })
                .clone(),
        }
    }
    /// Register a new callback for the `init` function. Does NOT overwrite any previous callbacks and just adds another.
    pub fn init(&self, f: impl FnMut(&FtcContext) + Send + 'static) {
        self.inner.lock().unwrap().init.push(Box::new(f));
    }
    /// Register a new callback for the `init_loop` function. Does NOT overwrite any previous callbacks and just adds another.
    pub fn init_loop(&self, f: impl FnMut(&FtcContext) + Send + 'static) {
        self.inner.lock().unwrap().init_loop.push(Box::new(f));
    }
    /// Register a new callback for the `start` function. Does NOT overwrite any previous callbacks and just adds another.
    pub fn start(&self, f: impl FnMut(&FtcContext) + Send + 'static) {
        self.inner.lock().unwrap().start.push(Box::new(f));
    }
    /// Register a new callback for the `loop` function. Does NOT overwrite any previous callbacks and just adds another.
    pub fn r#loop(&self, f: impl FnMut(&FtcContext) + Send + 'static) {
        self.inner.lock().unwrap().r#loop.push(Box::new(f));
    }
    /// Register a new callback for the `stop` function. Does NOT overwrite any previous callbacks and just adds another.
    pub fn stop(&self, f: impl FnMut(&FtcContext) + Send + 'static) {
        self.inner.lock().unwrap().stop.push(Box::new(f));
    }

    #[doc(hidden)]
    pub fn call_init(&mut self, ctx: &FtcContext) {
        self.inner.lock().unwrap().stage = OpModeStage::Init;
        for f in &mut self.inner.lock().unwrap().init {
            f(ctx);
        }
    }
    #[doc(hidden)]
    pub fn call_init_loop(&mut self, ctx: &FtcContext) {
        self.inner.lock().unwrap().stage = OpModeStage::Init;
        for f in &mut self.inner.lock().unwrap().init_loop {
            f(ctx);
        }
    }
    #[doc(hidden)]
    pub fn call_start(&mut self, ctx: &FtcContext) {
        self.inner.lock().unwrap().stage = OpModeStage::Running;
        for f in &mut self.inner.lock().unwrap().start {
            f(ctx);
        }
    }
    #[doc(hidden)]
    pub fn call_loop(&mut self, ctx: &FtcContext) {
        self.inner.lock().unwrap().stage = OpModeStage::Running;
        for f in &mut self.inner.lock().unwrap().r#loop {
            f(ctx);
        }
    }
    #[doc(hidden)]
    pub fn call_stop(&mut self, ctx: &FtcContext) {
        self.inner.lock().unwrap().stage = OpModeStage::Stop;
        for f in &mut self.inner.lock().unwrap().stop {
            f(ctx);
        }
    }
}

pub mod policy;

/// Better panic! that outputs a message through log since panics don't really work with the JNI
#[macro_export]
macro_rules! panic {
    () => {
        $crate::panic!("explicit panic");
    };
    ($($arg:tt)+) => {
        {
            let s = format!($($arg)*);
            $crate::log::error!("{s}");
            ::std::panic!("{s}");
        }
    };
}

/// Better unimplemented! that outputs a message through log since panics don't really work with the JNI
#[macro_export]
macro_rules! unimplemented {
    () => {
        $crate::panic!("not implemented")
    };
    ($($arg:tt)+) => {
        $crate::panic!("not implemented: {}", ::std::format_args!($($arg)+))
    };
}

/// Better todo! that outputs a message through log since panics don't really work with the JNI
#[macro_export]
macro_rules! todo {
    () => {
        $crate::panic!("not yet implemented")
    };
    ($($arg:tt)+) => {
        $crate::panic!("not yet implemented: {}", ::std::format_args!($($arg)+))
    };
}

/// Better unreachable! that outputs a message through log since panics don't really work with the JNI
#[macro_export]
macro_rules! unreachable {
    () => {
        $crate::panic!("nternal error: entered unreachable code")
    };
    ($($arg:tt)+) => {
        $crate::panic!("nternal error: entered unreachable code: {}", ::std::format_args!($($arg)+))
    };
}

/// Better assert! that outputs a message through log since panics don't really work with the JNI
#[macro_export]
macro_rules! assert {
    ($condition:expr $(,)?) => {
        {let cond: bool = $condition;
        if (!cond) {
            let s = concat!("assert at ", ::std::file!(), ":", ::std::line!(), ":", ::std::column!(), " failed");
            $crate::log::error!("{s}");
            ::std::panic!("{s}");
        }}
    };
    ($condition:expr, $($tt:tt)+) => {
        {let cond: bool = $condition;
        if (!cond) {
            let s = format!("{}{}", concat!("assert at ", ::std::file!(), ":", ::std::line!(), ":", ::std::column!(), " failed: "), format!($($tt)+));
            $crate::log::error!("{s}");
            ::std::panic!("{s}");
        }}
    };
}

/// Better debug_assert_eq! that outputs a message through log since panics don't really work with the JNI
#[macro_export]
macro_rules! assert_eq {
    ($val1:expr, $val2:expr $(,)?) => {
        $crate::assert!($val1 == $val2)
    };
    ($val1:expr, $val2:expr, $($tt:tt)+) => {
        $crate::assert!($val1 == $val2, $($tt)+)
    };
}

/// Better assert_ne! that outputs a message through log since panics don't really work with the JNI
#[macro_export]
macro_rules! assert_ne {
    ($val1:expr, $val2:expr $(,)?) => {
        $crate::assert!($val1 != $val2)
    };
    ($val1:expr, $val2:expr, $($tt:tt)+) => {
        $crate::assert!($val1 != $val2, $($tt)+)
    };
}

/// Better assert_matches! that outputs a message through log since panics don't really work with the JNI
#[macro_export]
macro_rules! assert_matches {
    ($expression:expr, $pattern:pat $(if $guard:expr)? $(,)?) => {
        $crate::assert!(matches!($expression, $pattern $(if $guard)?))
    };
    ($expression:expr, $pattern:pat $(if $guard:expr)?, $($tt:tt)+) => {
        $crate::assert!(matches!($expression, $pattern $(if $guard)?), $($tt)+)
    };
}

/// Better debug_assert! that outputs a message through log since panics don't really work with the JNI
#[macro_export]
macro_rules! debug_assert {
    ($condition:expr $(,)?) => {
        if ::std::cfg!(debug_assertions) {
            $crate::assert!($condition);
        }
    };
    ($condition:expr, $($tt:tt)+) => {
        if ::std::cfg!(debug_assertions) {
            $crate::assert!($condition, $($tt)+);
        }
    };
}

/// Better debug_assert_eq! that outputs a message through log since panics don't really work with the JNI
#[macro_export]
macro_rules! debug_assert_eq {
    ($val1:expr, $val2:expr $(,)?) => {
        if ::std::cfg!(debug_assertions) {
            $crate::assert!($val1 == $val2);
        }
    };
    ($val1:expr, $val2:expr, $($tt:tt)+) => {
        if ::std::cfg!(debug_assertions) {
            $crate::assert!($val1 == $val2, $($tt)+);
        }
    };
}

/// Better debug_assert_ne! that outputs a message through log since panics don't really work with the JNI
#[macro_export]
macro_rules! debug_assert_ne {
    ($val1:expr, $val2:expr $(,)?) => {
        if ::std::cfg!(debug_assertions) {
            $crate::assert!($val1 != $val2);
        }
    };
    ($val1:expr, $val2:expr, $($tt:tt)+) => {
        if ::std::cfg!(debug_assertions) {
            $crate::assert!($val1 != $val2, $($tt)+);
        }
    };
}

/// Better assert_matches! that outputs a message through log since panics don't really work with the JNI
#[macro_export]
macro_rules! debug_assert_matches {
    ($expression:expr, $pattern:pat $(if $guard:expr)? $(,)?) => {
        if ::std::cfg!(debug_assertions) {
            $crate::assert!(matches!($expression, $pattern $(if $guard)?));
        }
    };
    ($expression:expr, $pattern:pat $(if $guard:expr)?, $($tt:tt)+) => {
        if ::std::cfg!(debug_assertions) {
            $crate::assert!(matches!($expression, $pattern $(if $guard)?), $($tt)+);
        }
    };
}

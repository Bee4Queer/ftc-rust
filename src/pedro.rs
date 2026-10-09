//! Pedro pathing support.

use std::fmt::Debug;

use glam::{DVec2, dvec2};
use jni::{
    JValueOwned, jni_sig, jni_str, objects::JObject, refs::Global, strings::JNIString, vm::JavaVM,
};

use crate::{
    FtcContext, clone_global_ref, command::{Command, CommandHandle, SubsystemMap}, enum_variant_into, hardware::IntoJniObject,
};

/// The main Pedro Pathing struct. Needed to encapsulate a bunch of quirks of Pedro.
pub struct Pedro {
    follower: Global<JObject<'static>>,
    vm: JavaVM,
}

impl Clone for Pedro {
    fn clone(&self) -> Self {
        Self {
            follower: clone_global_ref(&self.vm, &self.follower),
            vm: self.vm.clone(),
        }
    }
}

impl Debug for Pedro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.vm
            .attach_current_thread(|env| {
                let log = env
                    .call_method(
                        &self.follower,
                        jni_str!("debug"),
                        jni_sig!("()Lcom/pedropathing/follower/FollowerLog;"),
                        &[],
                    )?
                    .l()?;
                let log = env
                    .call_method(
                        log,
                        jni_str!("toString"),
                        jni_sig!("()Ljava/lang/String;"),
                        &[],
                    )?
                    .l()?;
                let log = jni::objects::JString::cast_local(env, log)?;
                jni::errors::Result::Ok(write!(f, "Pedro(\n{log}\n)"))
            })
            .unwrap()
    }
}

impl Pedro {
    /// Pedro
    pub(crate) fn new(
        ftc: &FtcContext,
        constants_class: impl AsRef<str>,
        start_pose: Pose,
    ) -> Self {
        ftc.vm
            .attach_current_thread(|env| {
                let hardware_map = ftc.hardware().hardware_map;
                let hardware_map = env.new_local_ref(hardware_map)?;

                let constants_class = env.load_class(JNIString::new(constants_class))?;
                let follower = env
                    .call_static_method(
                        &constants_class,
                        jni_str!("create"),
                        jni_sig!(
                            "(Lcom/qualcomm/robotcore/hardware/HardwareMap;)Lcom/pedropathing/\
                             follower/Follower;"
                        ),
                        &[(&hardware_map).into()],
                    )?
                    .l()?;

                let start_pose = start_pose.into_jni_object(env);

                env.call_method(
                    &follower,
                    jni_str!("setPose"),
                    jni_sig!("(Lcom/pedropathing/math/Pose;)V"),
                    &[(&start_pose).into()],
                )?;

                jni::errors::Result::Ok(Self {
                    follower: env.new_global_ref(follower)?,
                    vm: ftc.vm.clone(),
                })
            })
            .unwrap()
    }
    /// Needs to be called every loop. Updates internal state for position of the robot.
    pub fn update(&self) {
        self.vm
            .attach_current_thread(|env| {
                env.call_method(&self.follower, jni_str!("update"), jni_sig!("()V"), &[])
                    .map(|_| ())
            })
            .unwrap();
    }

    /// Manual control from a joystick or similar.
    pub fn manual(&self, forward: f64, strafe: f64, turn: f64) {
        self.vm
            .attach_current_thread(|env| {
                env.call_method(
                    &self.follower,
                    jni_str!("manual"),
                    jni_sig!("(DDD)V"),
                    &[forward.into(), strafe.into(), turn.into()],
                )
                .map(|_| ())
            })
            .unwrap();
    }

    /// The current pose of the robot, as reported by Pedro Pathing.
    pub fn pose(&self) -> Pose {
        let pose = self
            .vm
            .attach_current_thread(|env| {
                env.call_method(
                    &self.follower,
                    jni_str!("pose"),
                    jni_sig!("()Lcom/pedropathing/math/Pose;"),
                    &[],
                )
                .and_then(|v| v.l())
                .and_then(|v| env.new_global_ref(v))
            })
            .unwrap();

        Pose::from_jni_object(&self.vm, pose)
    }
    /// Set the current pose of the robot to a known position.
    pub fn set_pose(&self, pose: Pose) {
        self.vm
            .attach_current_thread(|env| {
                let pose = pose.into_jni_object(env);
                env.call_method(
                    &self.follower,
                    jni_str!("setPose"),
                    jni_sig!("(Lcom/pedropathing/math/Pose;)V"),
                    &[(&pose).into()],
                )
                .map(|_| ())
            })
            .unwrap();
    }

    /// Get the current mode.
    pub fn mode(&self) -> Mode {
        Mode::from_jni_object(
            &self.vm,
            self.vm
                .attach_current_thread(|env| {
                    env.get_field(
                        &self.follower,
                        jni_str!("mode"),
                        jni_sig!("Lcom/pedropathing/follower/Follower$Mode;"),
                    )
                    .and_then(JValueOwned::l)
                    .and_then(|v| env.new_global_ref(v))
                })
                .unwrap(),
        )
    }

    /// Follow a path all the way to the end.
    pub fn follow(&self, path: impl AsRef<Path>) -> CommandHandle {
        FollowPathCommand {
            path: path.as_ref().clone(),
        }
        .schedule()
    }
    /// Hold a pose until another hold or follow command is sent.
    pub fn hold(&self, pose: Pose) -> CommandHandle {
        HoldPoseCommand {
            pedro: self.clone(),
            pose,
        }
        .schedule()
    }
    /// Stop the drive train.
    pub fn stop(&self) {
        self.vm
            .attach_current_thread(|env| {
                env.call_method(&self.follower, jni_str!("stop"), jni_sig!("()V"), &[])
                    .map(|_| ())
            })
            .unwrap();
    }
}

/// The current mode of the [Pedro Pathing follower](Pedro).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Actively following a path.
    Follow,
    /// Holding a specific pose.
    Hold,
    /// Being controlled with [`Pedro::manual`].
    Manual,
    /// Idle, doing nothing.
    Idle,
}

impl Mode {
    /// Whether this mode signifies the follower actively doing something.
    pub fn busy(self) -> bool {
        matches!(self, Mode::Follow | Mode::Hold)
    }
}

enum_variant_into! {
    Mode,
    "com/pedropathing/follower/Follower$Mode",
    "com.pedropathing.follower.Follower.Mode",
    Follow,
    Hold,
    Manual,
    Idle,
}

/// A pose in Pedro Pathing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[must_use]
pub struct Pose {
    /// In inches.
    pub pos: DVec2,
    /// In radians.
    pub heading: f64,
}

impl Pose {
    /// Create a new `Pose` from the provided X, Y, and heading. Heading is in degrees.
    pub fn new_degrees(x: f64, y: f64, heading: f64) -> Self {
        Self::new_radians(x, y, heading.to_radians())
    }
    /// Create a new `Pose` from the provided X, Y, and heading. Heading is in radians.
    pub fn new_radians(x: f64, y: f64, heading: f64) -> Self {
        Self {
            pos: dvec2(x, y),
            heading,
        }
    }
}

impl IntoJniObject for Pose {
    const JAVA_CLASS: &'static str = "com.pedropathing.math.Pose";
    const JNI_CLASS: &'static str = "com/pedropathing/math/Pose";
    fn into_jni_object<'local>(self, env: &mut jni::Env<'local>) -> JObject<'local> {
        let class = env
            .load_class(jni_str!("com/pedropathing/math/Pose"))
            .unwrap();

        env.new_object(
            class,
            jni_sig!("(DDD)Lcom/pedropathing/math/Pose;"),
            &[self.pos.x.into(), self.pos.y.into(), self.heading.into()],
        )
        .unwrap()
    }
    fn from_jni_object(vm: &JavaVM, obj: Global<JObject<'static>>) -> Self {
        vm.attach_current_thread(|env| {
            jni::errors::Result::Ok(Self {
                pos: DVec2 {
                    x: env.get_field(&obj, jni_str!("x"), jni_sig!("D"))?.d()?,
                    y: env.get_field(&obj, jni_str!("y"), jni_sig!("D"))?.d()?,
                },
                heading: env
                    .get_field(&obj, jni_str!("heading"), jni_sig!("D"))?
                    .d()?,
            })
        })
        .unwrap()
    }
}

/// A path that can be added on to
#[derive(Debug)]
pub struct Path {
    vm: JavaVM,
    path: Global<JObject<'static>>,
    pedro: Pedro,
}

impl Clone for Path {
    fn clone(&self) -> Self {
        Self {
            path: clone_global_ref(&self.vm, &self.path),
            vm: self.vm.clone(),
            pedro: self.pedro.clone(),
        }
    }
}

impl IntoJniObject for Path {
    const JAVA_CLASS: &'static str = "com.pedropathing.paths.Path";
    const JNI_CLASS: &'static str = "com/pedropathing/paths/Path";
    fn into_jni_object<'local>(self, env: &mut jni::Env<'local>) -> JObject<'local> {
        env.new_local_ref(&self.path).unwrap()
    }
    fn from_jni_object(_vm: &JavaVM, _obj: Global<JObject<'static>>) -> Self {
        unimplemented!()
    }
}

impl Pedro {
    /// Creates a straight line path from the start pose to the end pose.
    pub fn line(&self, start: Pose, end: Pose) -> Path {
        Path {
            vm: self.vm.clone(),
            path: self
                .vm
                .attach_current_thread(|env| {
                    let path_class = env.load_class(jni_str!("com/pedropathing/api/Paths"))?;
                    let start = start.into_jni_object(env);
                    let end = end.into_jni_object(env);
                    let path = env
                        .call_static_method(
                            &path_class,
                            JNIString::new("line"),
                            jni_sig!(
                                "(Lcom/pedropathing/math/Pose;Lcom/pedropathing/math/Pose;)Lcom/\
                                 pedropathing/paths/Path;"
                            ),
                            &[(&start).into(), (&end).into()],
                        )?
                        .l()?;
                    env.new_global_ref(path)
                })
                .unwrap(),
            pedro: self.clone(),
        }
    }
    /// Creates a Bézier curve.
    /// Requires at least 2 control poses.
    /// The first and last poses are the start and end of the curve, while the intermediate poses
    /// are control poses.
    pub fn curve(&self, poses: impl AsRef<[Pose]>) -> Path {
        Path {
            vm: self.vm.clone(),
            path: self
                .vm
                .attach_current_thread(|env| {
                    let path_class = env.load_class(jni_str!("com/pedropathing/api/Paths"))?;
                    let poses = crate::jlist![env env; from poses.as_ref()];
                    let path = env
                        .call_static_method(
                            &path_class,
                            JNIString::new("curve"),
                            jni_sig!(
                                "([Lcom/pedropathing/math/Pose;)Lcom/pedropathing/paths/Path;"
                            ),
                            &[(&poses).into()],
                        )?
                        .l()?;
                    env.new_global_ref(path)
                })
                .unwrap(),
            pedro: self.clone(),
        }
    }
}

impl AsRef<Path> for Path {
    fn as_ref(&self) -> &Path {
        self
    }
}

impl Path {
    /// Combine the listed paths.
    #[track_caller]
    pub fn combine(others: impl AsRef<[Path]>) -> Path {
        let this = others
            .as_ref()
            .first()
            .expect("cannot combine empty list of paths");
        let obj = this
            .vm
            .attach_current_thread(|env| {
                let path_class = env.load_class(jni_str!("com/pedropathing/api/Paths"))?;
                let from = others
                    .as_ref()
                    .iter()
                    .map(|path| env.new_local_ref(&path.path))
                    .collect::<Result<Vec<_>, _>>()?;

                let poses = crate::jlist![env env; from objs from];
                let path = env
                    .call_static_method(
                        &path_class,
                        JNIString::new("path"),
                        jni_sig!("([Lcom/pedropathing/paths/Path;)Lcom/pedropathing/paths/Path;"),
                        &[(&poses).into()],
                    )?
                    .l()?;
                env.new_global_ref(path)
            })
            .unwrap();

        Path {
            vm: this.vm.clone(),
            path: obj,
            pedro: this.pedro.clone(),
        }
    }
    /// Creates a straight line path from the start pose to the end pose.
    pub fn line(self, start: Pose, end: Pose) -> Path {
        let other = self.pedro.line(start, end);

        Self::combine([self, other])
    }
    /// Creates a Bézier curve.
    /// Requires at least 2 control poses.
    /// The first and last poses are the start and end of the curve, while the intermediate poses
    /// are control poses.
    pub fn curve(self, poses: impl AsRef<[Pose]>) -> Path {
        let other = self.pedro.curve(poses);

        Self::combine([self, other])
    }

    /// Set heading interpolation to be linear from `start_heading` to `end_heading`. The provided
    /// headings are in radians.
    #[doc(alias = "linear")]
    pub fn heading_linear(self, start_heading: f64, end_heading: f64) -> Path {
        let path = self
            .vm
            .attach_current_thread(|env| {
                env.call_method(
                    &self.path,
                    jni_str!("linear"),
                    jni_sig!("(DD)Lcom/pedropathing/paths/Path;"),
                    &[start_heading.into(), end_heading.into()],
                )
                .and_then(JValueOwned::l)
                .and_then(|v| env.new_global_ref(v))
            })
            .unwrap();

        Path {
            vm: self.vm.clone(),
            path,
            pedro: self.pedro.clone(),
        }
    }

    /// Set heading interpolation to be tangent. Basically this means that the robot will face in
    /// the direction of travel.
    #[doc(alias = "tangent")]
    pub fn heading_tangent(self) -> Path {
        let path = self
            .vm
            .attach_current_thread(|env| {
                env.call_method(
                    &self.path,
                    jni_str!("tangent"),
                    jni_sig!("()Lcom/pedropathing/paths/Path;"),
                    &[],
                )
                .and_then(JValueOwned::l)
                .and_then(|v| env.new_global_ref(v))
            })
            .unwrap();

        Path {
            vm: self.vm.clone(),
            path,
            pedro: self.pedro.clone(),
        }
    }

    /// Set the heading to be constant throughout the entire path. The provided heading is in
    /// radians.
    #[doc(alias = "constant")]
    pub fn heading_constant(self, heading: f64) -> Path {
        let path = self
            .vm
            .attach_current_thread(|env| {
                env.call_method(
                    &self.path,
                    jni_str!("constant"),
                    jni_sig!("(D)Lcom/pedropathing/paths/Path;"),
                    &[heading.into()],
                )
                .and_then(JValueOwned::l)
                .and_then(|v| env.new_global_ref(v))
            })
            .unwrap();

        Path {
            vm: self.vm.clone(),
            path,
            pedro: self.pedro.clone(),
        }
    }
    /// Face a specific point while traveling.
    #[doc(alias = "facingPoint")]
    pub fn heading_face(self, facing: DVec2) -> Path {
        let path = self
            .vm
            .attach_current_thread(|env| {
                let pose = Pose {
                    pos: facing,
                    heading: 0.0,
                }
                .into_jni_object(env);
                env.call_method(
                    &self.path,
                    jni_str!("facingPoint"),
                    jni_sig!("(D)Lcom/pedropathing/paths/Path;"),
                    &[(&pose).into()],
                )
                .and_then(JValueOwned::l)
                .and_then(|v| env.new_global_ref(v))
            })
            .unwrap();

        Path {
            vm: self.vm.clone(),
            path,
            pedro: self.pedro.clone(),
        }
    }
}

struct FollowPathCommand {
    path: Path,
}

impl Command for FollowPathCommand {
    fn init(&mut self, ctx: &FtcContext, _: &mut SubsystemMap) {
        ctx.vm
            .attach_current_thread(|env| {
                let path = env.new_local_ref(&self.path.path)?;
                env.call_method(
                    &self.path.pedro.follower,
                    jni_str!("follow"),
                    jni_sig!("(Lcom/pedropathing/paths/Path;)V"),
                    &[(&path).into()],
                )?;
                jni::errors::Result::Ok(())
            })
            .unwrap();
    }
    fn is_finished(&mut self, ctx: &FtcContext, _: &mut SubsystemMap) -> bool {
        ctx.vm
            .attach_current_thread(|env| {
                env.call_method(
                    &self.path.pedro.follower,
                    jni_str!("atParametricEnd"),
                    jni_sig!("()Z"),
                    &[],
                )
                .and_then(JValueOwned::z)
            })
            .unwrap()
    }
}

struct HoldPoseCommand {
    pedro: Pedro,
    pose: Pose,
}

impl Command for HoldPoseCommand {
    fn init(&mut self, ctx: &FtcContext, _: &mut SubsystemMap) {
        ctx.vm
            .attach_current_thread(|env| {
                let pose = self.pose.into_jni_object(env);
                env.call_method(
                    &self.pedro.follower,
                    jni_str!("hold"),
                    jni_sig!("(Lcom/pedropathing/math/Pose;)V"),
                    &[(&pose).into()],
                )?;
                jni::errors::Result::Ok(())
            })
            .unwrap();
    }
    fn is_finished(&mut self, _ctx: &FtcContext, _: &mut SubsystemMap) -> bool {
        self.pedro.mode() != Mode::Hold
    }
}

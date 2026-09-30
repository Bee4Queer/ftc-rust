//! Pedro pathing support.

use jni::{
    jni_sig, jni_str,
    objects::{JClass, JObject},
    refs::Global,
    strings::JNIString,
    vm::JavaVM,
};

use crate::{FtcContext, hardware::IntoJniObject};

/// The main Pedro Pathing struct. Needed to encapsulate a bunch of quirks of Pedro.
#[derive(Debug)]
pub struct Pedro {
    constants: Global<JClass<'static>>,
    follower: Global<JObject<'static>>,
    vm: JavaVM,
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
                let follower = env.call_static_method(
                    &constants_class,
                    jni_str!("create"),
                    jni_sig!("(Lcom/qualcomm/robotcore/hardware/HardwareMap;)Lcom/pedropathing/follower/Follower;"),
                    &[(&hardware_map).into()],
                )?.l()?;

                let start_pose = start_pose.into_jni_object(env);

                env.call_method(
                    &follower,
                    jni_str!("setPose"),
                    jni_sig!("(Lcom/pedropathing/math/Pose;)V"),
                    &[(&start_pose).into()],
                )?;

                jni::errors::Result::Ok(Self {
                    follower: env.new_global_ref(follower)?,
                    constants: env.new_global_ref(constants_class)?,
                    vm: ftc.vm.clone()
                })
            })
            .unwrap()
    }
}

/// A pose in Pedro Pathing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// In inches.
    pub x: f64,
    /// In inches.
    pub y: f64,
    /// In radians.
    pub heading: f64,
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
            &[self.x.into(), self.y.into(), self.heading.into()],
        )
        .unwrap()
    }
    fn from_jni_object(vm: &JavaVM, obj: Global<JObject<'static>>) -> Self {
        vm.attach_current_thread(|env| {
            jni::errors::Result::Ok(Self {
                x: env.get_field(&obj, jni_str!("x"), jni_sig!("D"))?.d()?,
                y: env.get_field(&obj, jni_str!("y"), jni_sig!("D"))?.d()?,
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
}

impl Pedro {
    /// Creates a straight line path from the start pose to the end pose.
    pub fn line(&self, start: Pose, end: Pose) -> Path {
        Path {
            vm: self.vm.clone(),
            path: self
                .vm
                .attach_current_thread(|env| {
                    let path_class = env
                        .load_class(jni_str!("com/pedropathing/api/Paths"))?;
                    let start = start.into_jni_object(env);
                    let end = end.into_jni_object(env);
                    let path = env
                        .call_static_method(
                            &path_class,
                            JNIString::new("line"),
                            jni_sig!("(Lcom/pedropathing/math/Pose;Lcom/pedropathing/math/Pose;)Lcom/pedropathing/paths/Path;"),
                            &[(&start).into(), (&end).into()],
                        )?
                        .l()?;
                    env.new_global_ref(path)
                })
                .unwrap(),
        }
    }
    /// Creates a Bézier curve.
    /// Requires at least 2 control poses.
    /// The first and last poses are the start and end of the curve, while the intermediate poses are control poses.
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
        }
    }
}

impl Path {
    /// Creates a straight line path from the start pose to the end pose.
    pub fn line(&mut self, start: Pose, end: Pose) -> &mut Self {
        self.path = self
            .vm
            .attach_current_thread(|env| {
                let start = start.into_jni_object(env);
                let end = end.into_jni_object(env);
                let path = env
                    .call_method(
                        &self.path,
                        JNIString::new("line"),
                        jni_sig!("(Lcom/pedropathing/math/Pose;Lcom/pedropathing/math/Pose;)Lcom/pedropathing/paths/Path;"),
                        &[(&start).into(), (&end).into()],
                    )?
                    .l()?;
                env.new_global_ref(path)
            })
            .unwrap();
        self
    }
    /// Creates a Bézier curve.
    /// Requires at least 2 control poses.
    /// The first and last poses are the start and end of the curve, while the intermediate poses are control poses.
    pub fn curve(&mut self, poses: impl AsRef<[Pose]>) -> &mut Self {
        self.path = self
            .vm
            .attach_current_thread(|env| {
                let path_class = env.load_class(jni_str!("com/pedropathing/api/Paths"))?;
                let poses = crate::jlist![env env; from poses.as_ref()];
                let path = env
                    .call_static_method(
                        &path_class,
                        JNIString::new("curve"),
                        jni_sig!("([Lcom/pedropathing/math/Pose;)Lcom/pedropathing/paths/Path;"),
                        &[(&poses).into()],
                    )?
                    .l()?;
                env.new_global_ref(path)
            })
            .unwrap();
        self
    }
}

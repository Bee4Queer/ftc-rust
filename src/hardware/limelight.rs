//! Limelight vision.

use std::time::Duration;

use glam::{Mat4, Quat, vec4};
use jni::{
    JValue, jni_sig,
    objects::{JList, JObject, JString},
    refs::Reference,
    strings::JNIString,
};

use crate::{
    call_method, debug_assert, get_field, hardware::IntoJniObject, index_jlist, jlist, panic, todo,
};

/// Javadoc available at <https://javadoc.io/static/org.firstinspires.ftc/Hardware/11.1.0/com/qualcomm/hardware/limelightvision/Limelight3A.html>.
///
/// Driver for Limelight 3A Vision Sensor. `Limelight3A` provides support for the Limelight Vision
/// Limelight 3A Vision Sensor.
#[doc(hidden)]
struct Limelight3AInner {
    /// The environment.
    vm: crate::jni::JavaVM,
    /// The actual object.
    object: crate::jni::refs::Global<crate::jni::objects::JObject<'static>>,
}

impl std::fmt::Debug for Limelight3AInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            "(opaque Limelight3A object, wraps \
             com.qualcomm.robotcore.hardware.limelightvision.Limelight3A)",
        )
    }
}

/// Javadoc available at <https://javadoc.io/static/org.firstinspires.ftc/Hardware/11.1.0/com/qualcomm/hardware/limelightvision/Limelight3A.html>.
///
/// Driver for Limelight 3A Vision Sensor. `Limelight3A` provides support for the Limelight Vision
/// Limelight 3A Vision Sensor.
///
/// Default is essentially a null pointer and will panic upon attempted use.
#[repr(transparent)]
#[derive(Default)]
pub struct Limelight3A {
    #[allow(clippy::missing_docs_in_private_items)]
    inner: Option<Limelight3AInner>,
}

#[allow(clippy::missing_docs_in_private_items)]
impl Limelight3A {
    /// Returns whether this device is a null pointer.
    #[must_use]
    pub fn is_null(&self) -> bool {
        self.inner.is_none()
    }
    #[must_use]
    fn vm(&self) -> &crate::jni::JavaVM {
        if let Some(inner) = self.inner.as_ref() {
            &inner.vm
        } else {
            panic!("Attempted to use null device");
        }
    }
    #[must_use]
    fn object(&self) -> &crate::jni::refs::Global<crate::jni::objects::JObject<'static>> {
        if let Some(inner) = self.inner.as_ref() {
            &inner.object
        } else {
            panic!("Attempted to use null device");
        }
    }
}

impl std::fmt::Debug for Limelight3A {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            "(opaque Limelight3A object, wraps \
             com.qualcomm.robotcore.hardware.limelightvision.Limelight3A)",
        )
    }
}

impl crate::hardware::Device for Limelight3A {
    const JAVA_CLASS: &'static str = "com.qualcomm.robotcore.hardware.limelightvision.Limelight3A";
    const JNI_CLASS: &'static str = "com/qualcomm/robotcore/hardware/limelightvision/Limelight3A";
    fn from_java(
        vm: crate::jni::JavaVM,
        object: crate::jni::refs::Global<crate::jni::objects::JObject<'static>>,
    ) -> Self {
        let out = Self {
            inner: Some(Limelight3AInner { vm, object }),
        };
        out.start();
        out
    }
}

/// Temporary configuration struct used for changing configuration of a [`Limelight3A`]. Shouldn't
/// be stored usually, as it prevents usage of the base `Limelight3A` instance until it is dropped.
pub struct Limelight3AConfig<'a> {
    #[allow(clippy::missing_docs_in_private_items)]
    limelight: &'a Limelight3A,
}

impl std::fmt::Debug for Limelight3AConfig<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            "(opaque Limelight3AConfig object, wraps \
             com.qualcomm.robotcore.hardware.limelightvision.Limelight3A)",
        )
    }
}

impl Limelight3AConfig<'_> {
    /// Sets the poll rate in Hertz (Hz). The rate must be between 1 and 250 inclusive.
    #[doc(alias = "setPollRateHz")]
    pub fn set_poll_rate(&self, hz: u8) {
        debug_assert!((1..=250).contains(&hz));
        self.limelight
            .vm()
            .attach_current_thread(|env| {
                call_method!(
                    env env,
                    self.limelight.object(),
                    "setPollRateHz",
                    "(I)V",
                    [i32::from(hz)]
                )?;
                jni::errors::Result::Ok(())
            })
            .unwrap();
    }
}

impl Drop for Limelight3AConfig<'_> {
    fn drop(&mut self) {
        self.limelight.start();
    }
}

impl Limelight3A {
    /// Configure
    pub fn config<R>(&self, f: impl FnOnce(&Limelight3AConfig) -> R) -> R {
        self.stop();
        let cfg = Limelight3AConfig { limelight: self };
        f(&cfg)
    }
    /// Starts or resumes periodic polling of Limelight data.
    ///
    /// Mostly unnecessary with the Rust SDK due to how stuff is implemented.
    pub fn start(&self) {
        self.vm()
            .attach_current_thread(|env| {
                call_method!(
                    env env,
                    self.object(),
                    "start",
                    "()V",
                    []
                )?;
                jni::errors::Result::Ok(())
            })
            .unwrap();
    }
    /// Stops polling of Limelight data.
    ///
    /// Mostly unnecessary with the Rust SDK due to how stuff is implemented.
    pub fn stop(&self) {
        self.vm()
            .attach_current_thread(|env| {
                call_method!(
                    env env,
                    self.object(),
                    "stop",
                    "()V",
                    []
                )?;
                jni::errors::Result::Ok(())
            })
            .unwrap();
    }
    /// Pauses polling of Limelight data.
    pub fn pause(&self) {
        self.vm()
            .attach_current_thread(|env| {
                call_method!(
                    env env,
                    self.object(),
                    "pause",
                    "()V",
                    []
                )?;
                jni::errors::Result::Ok(())
            })
            .unwrap();
    }
    /// Shuts down the Limelight connection and stops all ongoing processes.
    pub fn shutdown(&self) {
        self.vm()
            .attach_current_thread(|env| {
                call_method!(
                    env env,
                    self.object(),
                    "shutdown",
                    "()V",
                    []
                )?;
                jni::errors::Result::Ok(())
            })
            .unwrap();
    }

    /// Checks if polling is enabled.
    #[doc(alias = "isRunning")]
    #[must_use]
    pub fn running(&self) -> bool {
        self.vm()
            .attach_current_thread(|env| {
                call_method!(
                    env env,
                    self.object(),
                    "isRunning",
                    "()Z",
                    []
                )
                .and_then(jni::JValueOwned::z)
            })
            .unwrap()
    }

    /// Gets the time elapsed since the last update.
    #[doc(alias = "getTimeSinceLastUpdate")]
    #[must_use]
    pub fn time_since_update(&self) -> Duration {
        Duration::from_millis(
            self.vm()
                .attach_current_thread(|env| {
                    call_method!(
                        env env,
                        self.object(),
                        "getTimeSinceLastUpdate",
                        "()J",
                        []
                    )
                    .and_then(jni::JValueOwned::j)
                })
                .unwrap() as u64,
        )
    }

    /// Checks if the Limelight is currently connected.
    #[doc(alias = "isConnected")]
    #[must_use]
    pub fn connected(&self) -> bool {
        self.vm()
            .attach_current_thread(|env| {
                call_method!(
                    env env,
                    self.object(),
                    "isConnected",
                    "()Z",
                    []
                )
                .and_then(jni::JValueOwned::z)
            })
            .unwrap()
    }

    /// Reloads the current Limelight pipeline.
    ///
    /// Literally no idea what the returned boolean is.
    #[doc(alias = "reloadPipeline")]
    #[must_use]
    pub fn reload_pipeline(&self) -> bool {
        self.vm()
            .attach_current_thread(|env| {
                call_method!(
                    env env,
                    self.object(),
                    "reloadPipeline",
                    "()Z",
                    []
                )
                .and_then(jni::JValueOwned::z)
            })
            .unwrap()
    }

    /// Switches to a pipeline at the specified index.
    ///
    /// Literally no idea what the returned boolean is.
    #[doc(alias = "pipelineSwitch")]
    #[must_use]
    pub fn switch_pipeline(&self, index: usize) -> bool {
        self.vm()
            .attach_current_thread(|env| {
                call_method!(
                    env env,
                    self.object(),
                    "pipelineSwitch",
                    "(I)Z",
                    [index as i32]
                )
                .and_then(jni::JValueOwned::z)
            })
            .unwrap()
    }

    /// Captures a snapshot with the given name.
    ///
    /// Literally no idea what the returned boolean is.
    #[doc(alias = "captureSnapshot")]
    #[must_use]
    pub fn capture_snapshot(&self, name: impl AsRef<str>) -> bool {
        self.vm()
            .attach_current_thread(|env| {
                let s = JString::new(env, name).unwrap();
                call_method!(
                    env env,
                    self.object(),
                    "captureSnapshot",
                    "(Ljava/lang/String;)Z",
                    [AsRef::<JObject>::as_ref(&s)]
                )
                .and_then(jni::JValueOwned::z)
            })
            .unwrap()
    }

    /// Deletes all snapshots.
    ///
    /// Literally no idea what the returned boolean is.
    #[doc(alias = "deleteSnapshots")]
    #[must_use]
    pub fn delete_snapshots(&self) -> bool {
        self.vm()
            .attach_current_thread(|env| {
                call_method!(
                    env env,
                    self.object(),
                    "deleteSnapshots",
                    "()Z",
                    []
                )
                .and_then(jni::JValueOwned::z)
            })
            .unwrap()
    }

    /// Deletes a specific snapshot.
    ///
    /// Literally no idea what the returned boolean is.
    #[doc(alias = "deleteSnapshot")]
    #[must_use]
    pub fn delete_snapshot(&self, name: impl AsRef<str>) -> bool {
        self.vm()
            .attach_current_thread(|env| {
                let s = JString::new(env, name).unwrap();
                call_method!(
                    env env,
                    self.object(),
                    "deleteSnapshot",
                    "(Ljava/lang/String;)Z",
                    [AsRef::<JObject>::as_ref(&s)]
                )
                .and_then(jni::JValueOwned::z)
            })
            .unwrap()
    }

    #[allow(clippy::doc_markdown)]
    /// Updates the Python SnapScript inputs.
    ///
    /// Literally no idea what the returned boolean is.
    #[doc(alias = "updatePythonInputs")]
    #[must_use]
    pub fn update_python_inputs(&self, values: [f64; 8]) -> bool {
        self.vm()
            .attach_current_thread(|env| {
                call_method!(
                    env env,
                    self.object(),
                    "updatePythonInputs",
                    "(DDDDDDDD)Z",
                    values
                )
                .and_then(jni::JValueOwned::z)
            })
            .unwrap()
    }

    #[allow(clippy::doc_markdown)]
    /// Updates the robot orientation for MegaTag2. Yaw value should be aligned with the field map.
    ///
    /// Literally no idea what the returned boolean is.
    #[doc(alias = "updateRobotOrientation")]
    #[must_use]
    pub fn update_robot_orientation(&self, yaw: f64) -> bool {
        self.vm()
            .attach_current_thread(|env| {
                call_method!(
                    env env,
                    self.object(),
                    "updateRobotOrientation",
                    "(D)Z",
                    [yaw]
                )
                .and_then(jni::JValueOwned::z)
            })
            .unwrap()
    }

    /// Uploads a pipeline to a specific slot.
    ///
    /// Literally no idea what the returned boolean is.
    #[doc(alias = "uploadPipeline")]
    #[must_use]
    pub fn upload_pipeline(&self, json: impl AsRef<str>, index: Option<usize>) -> bool {
        self.vm()
            .attach_current_thread(|env| {
                let s = JString::new(env, json).unwrap();
                call_method!(
                    env env,
                    self.object(),
                    "uploadPipeline",
                    "(Ljava/lang/String;I)Z",
                    [AsRef::<JObject>::as_ref(&s).into(), match index {
                        None => JValue::Void,
                        Some(v) => JValue::Int(v as i32),
                    }]
                )
                .and_then(jni::JValueOwned::z)
            })
            .unwrap()
    }

    /// Gets the current status of the Limelight.
    #[doc(alias = "getStatus")]
    pub fn status(&self) -> LLStatus {
        let global = self
            .vm()
            .attach_current_thread(|env| {
                call_method!(
                    env env,
                    self.object(),
                    "getStatus",
                    format!("()L{};", LLStatus::JNI_CLASS),
                    []
                )
                .and_then(jni::JValueOwned::l)
                .and_then(|v| env.new_global_ref(v))
            })
            .unwrap();

        LLStatus::from_jni_object(self.vm(), global)
    }

    /// Uploads a new fiducial field map. Panics in debug builds if map is empty or doesn't specify
    /// a type.
    #[doc(alias = "uploadFieldmap")]
    #[must_use]
    pub fn upload_fieldmap(&self, map: LLFieldMap, index: usize) -> bool {
        debug_assert!(!map.ty.is_empty(), "Field map has no type");
        debug_assert!(
            !map.fiducials.is_empty(),
            "Field map fiducials list is empty"
        );

        self.vm()
            .attach_current_thread(|env| {
                let map = map.into_jni_object(env);
                call_method!(
                    env env,
                    self.object(),
                    "uploadFieldmap",
                    "(Lcom/qualcomm/hardware/limelightvision/LLFieldMap;I)Z",
                    [(&map).into(), JValue::Int(index as i32)]
                )
                .and_then(jni::JValueOwned::z)
            })
            .unwrap()
    }

    /// Uploads new Python code.
    #[doc(alias = "uploadPython")]
    #[must_use]
    pub fn upload_python(&self, code: String, index: Option<usize>) -> bool {
        self.vm()
            .attach_current_thread(|env| {
                let s = JString::new(env, code).unwrap();
                call_method!(
                    env env,
                    self.object(),
                    "uploadPython",
                    "(Ljava/lang/String;I)Z",
                    [AsRef::<JObject>::as_ref(&s).into(), match index {
                        None => JValue::Void,
                        Some(v) => JValue::Int(v as i32),
                    }]
                )
                .and_then(jni::JValueOwned::z)
            })
            .unwrap()
    }
}

impl Drop for Limelight3A {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Represents the status of a Limelight.
#[derive(Clone, Debug, PartialEq)]
#[must_use]
#[allow(
    missing_docs,
    reason = "the fields aren't documented on the base LLStatus object"
)]
pub struct LLStatus {
    pub camera_quat: Quat,
    pub cid: i32,
    pub cpu: f64,
    pub final_yaw: f64,
    pub fps: f64,
    pub hw_type: i32,
    pub name: String,
    pub pipe_img_count: usize,
    pub pipeline_index: usize,
    pub pipeline_type: String,
    pub ram: f64,
    pub snapshot_mode: i32,
    pub temp: f64,
}

impl IntoJniObject for LLStatus {
    const JAVA_CLASS: &'static str = "com.qualcomm.hardware.limelightvision.LLStatus";
    const JNI_CLASS: &'static str = "com/qualcomm/hardware/limelightvision/LLStatus";
    fn from_jni_object(
        vm: &jni::vm::JavaVM,
        obj: jni::refs::Global<jni::objects::JObject<'static>>,
    ) -> Self {
        vm.attach_current_thread(|env| {
            let camera_quat = get_field!(obj env, &obj, "cameraQuat", "Lorg/firstinspires/ftc/robotcore/external/navigation/Quaternion;");
            jni::errors::Result::Ok(LLStatus {
                camera_quat: Quat::from_jni_object(vm, camera_quat),
                cid: get_field!(int env, &obj, "cid"),
                cpu: get_field!(double env, &obj, "cpu"),
                final_yaw: get_field!(double env, &obj, "finalYaw"),
                fps: get_field!(double env, &obj, "fps"),
                hw_type: get_field!(int env, &obj, "hwType"),
                name: get_field!(str env, &obj, "name"),
                pipe_img_count: get_field!(int env, &obj, "pipeImgCount") as usize,
                pipeline_index: get_field!(int env, &obj, "pipelineIndex") as usize,
                pipeline_type: get_field!(str env, &obj, "pipelineType"),
                ram: get_field!(double env, &obj, "ram"),
                snapshot_mode: get_field!(int env, &obj, "snapshotMode"),
                temp: get_field!(double env, &obj, "temp"),
            })
        })
        .unwrap()
    }
    fn into_jni_object<'local>(self, _env: &mut jni::Env<'local>) -> jni::objects::JObject<'local> {
        todo!();
    }
}

#[allow(clippy::doc_markdown)]
/// Represents a fiducial marker / AprilTag in the field map.
#[derive(Clone, Debug, PartialEq)]
#[must_use]
pub struct LLFieldMapFiducial {
    /// The ID / index of the fiducial.
    pub id: usize,
    /// The size of the fiducial in millimeters
    pub size: f64,
    /// The family of the fiducial. For example, `apriltag3_36h11_classic`
    pub family: String,
    /// The 4x4 transforms matrix of the fiducial.
    pub transform: Mat4,
    /// Is the fiduical unique?
    pub unique: bool,
}

impl IntoJniObject for LLFieldMapFiducial {
    const JAVA_CLASS: &'static str = "com.qualcomm.hardware.limelightvision.LLFieldMap.Fiducial";
    const JNI_CLASS: &'static str = "com/qualcomm/hardware/limelightvision/LLFieldMap$Fiducial";
    fn from_jni_object(vm: &jni::vm::JavaVM, obj: jni::refs::Global<JObject<'static>>) -> Self {
        vm.attach_current_thread(|env| {
            let transform = get_field!(local_obj env, &obj, "transform", "Ljava/util/List;");
            let transform = JList::cast_local(env, transform)?;
            jni::errors::Result::Ok(LLFieldMapFiducial {
                id: get_field!(int env, &obj, "id") as usize,
                size: get_field!(double env, &obj, "size"),
                family: get_field!(str env, &obj, "family"),
                unique: get_field!(bool env, &obj, "unique"),
                transform: Mat4::from_cols(
                    vec4(
                        index_jlist!(float env, transform; [0]),
                        index_jlist!(float env, transform; [1]),
                        index_jlist!(float env, transform; [2]),
                        index_jlist!(float env, transform; [3]),
                    ),
                    vec4(
                        index_jlist!(float env, transform; [4]),
                        index_jlist!(float env, transform; [5]),
                        index_jlist!(float env, transform; [6]),
                        index_jlist!(float env, transform; [7]),
                    ),
                    vec4(
                        index_jlist!(float env, transform; [8]),
                        index_jlist!(float env, transform; [9]),
                        index_jlist!(float env, transform; [10]),
                        index_jlist!(float env, transform; [11]),
                    ),
                    vec4(
                        index_jlist!(float env, transform; [12]),
                        index_jlist!(float env, transform; [13]),
                        index_jlist!(float env, transform; [14]),
                        index_jlist!(float env, transform; [15]),
                    ),
                ),
            })
        })
        .unwrap()
    }
    fn into_jni_object<'local>(self, env: &mut jni::Env<'local>) -> JObject<'local> {
        let class = env.find_class(JNIString::new(Self::JNI_CLASS)).unwrap();
        let family = JString::new(env, self.family).unwrap();
        #[rustfmt::skip]
        let transform = jlist![
            float env;
            self.transform.x_axis.x, self.transform.x_axis.y, self.transform.x_axis.z, self.transform.x_axis.w,
            self.transform.y_axis.x, self.transform.y_axis.y, self.transform.y_axis.z, self.transform.y_axis.w,
            self.transform.z_axis.x, self.transform.z_axis.y, self.transform.z_axis.z, self.transform.z_axis.w,
            self.transform.w_axis.x, self.transform.w_axis.y, self.transform.w_axis.z, self.transform.w_axis.w,
        ];

        env.new_object(
            class,
            jni_sig!(
                "(IDLjava/lang/String;Ljava/util/List;Z)Lcom/qualcomm/hardware/limelightvision/\
                 LLFieldMap$Fiducial;"
            ),
            &[
                (self.id as i32).into(),
                (self.size).into(),
                (&family).into(),
                (&transform).into(),
            ],
        )
        .unwrap()
    }
}

#[allow(clippy::doc_markdown)]
/// Represents a field map containing fiducial markers / AprilTags.
#[derive(Clone, Debug, PartialEq)]
#[must_use]
pub struct LLFieldMap {
    /// The list of fiducials in the field map.
    pub fiducials: Vec<LLFieldMapFiducial>,
    /// The type of the field map (e.g. "ftc" or "frc").
    #[doc(alias = "type")]
    pub ty: String,
}

impl IntoJniObject for LLFieldMap {
    const JAVA_CLASS: &'static str = "com.qualcomm.hardware.limelightvision.LLFieldMap";
    const JNI_CLASS: &'static str = "com/qualcomm/hardware/limelightvision/LLFieldMap";
    fn from_jni_object(vm: &jni::vm::JavaVM, obj: jni::refs::Global<JObject<'static>>) -> Self {
        let (ty, fiducials) = vm
            .attach_current_thread(|env| {
                let fiducials = get_field!(local_obj env, &obj, "fiducials", "Ljava/util/List;");
                let fiducials = JList::cast_local(env, fiducials)?.iter(env)?;
                let mut out_fiducials = Vec::new();

                while let Some(v) = fiducials.next(env)? {
                    out_fiducials.push(env.new_global_ref(v)?);
                }

                jni::errors::Result::Ok((get_field!(str env, &obj, "type"), out_fiducials))
            })
            .unwrap();

        Self {
            ty,
            fiducials: fiducials
                .into_iter()
                .map(|v| LLFieldMapFiducial::from_jni_object(vm, v))
                .collect(),
        }
    }
    fn into_jni_object<'local>(self, env: &mut jni::Env<'local>) -> JObject<'local> {
        let class = env.find_class(JNIString::new(Self::JNI_CLASS)).unwrap();
        let fiducials = jlist![
            env env;
            from self.fiducials
        ];
        let ty = JString::new(env, self.ty).unwrap();
        env.new_object(
            class,
            jni_sig!(
                "(Ljava/util/List;Ljava/lang/String;)Lcom/qualcomm/hardware/limelightvision/\
                 LLFieldMap;"
            ),
            &[(&fiducials).into(), (&ty).into()],
        )
        .unwrap()
    }
}

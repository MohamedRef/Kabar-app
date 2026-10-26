//! Android bridge: reaches Java/Kotlin through JNI from Rust, using the
//! JavaVM and Activity that tao/wry register in `ndk_context`.
//!
//! Framework classes are found directly. Our own Kotlin classes (`android/`,
//! package `app.kabar`) are loaded through the Activity's class loader, because
//! `FindClass` from a native thread only sees the system class loader.

#![allow(unsafe_code)]

use std::path::Path;

use jni::objects::{JClass, JClassLoader, JObject, JString};
use jni::refs::LoaderContext;
use jni::strings::JNIStr;
use jni::{Env, JValue, JavaVM, jni_sig, jni_str};
use kabar_protocol::reader::{OcrResult, SharedInput};

/// Tells `dx` to build `android/` (repo root) into the APK: it copies the folder
/// into its generated Gradle project as the library module `:plugins:plugin`,
/// adds it as an app dependency (ML Kit comes with it) and the manifest merger
/// adds our share activity. The macro only embeds that metadata in the binary;
/// it declares no functions, the JNI calls below are hand-written.
mod kotlin_module {
    #![allow(unused_imports)]

    #[manganis::ffi("../../android")]
    extern "Kotlin" {}
}

fn with_activity<T>(f: impl FnOnce(&mut Env, &JObject) -> jni::errors::Result<T>) -> Result<T, String> {
    let ctx = ndk_context::android_context();
    // SAFETY: tao initializes ndk_context with the process JavaVM and a global
    // reference to the Activity before any app code runs; both live for the
    // whole process.
    let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) };
    vm.attach_current_thread(|env| {
        // SAFETY: see above; the Activity is a valid global reference.
        let activity = unsafe { JObject::from_raw(env, ctx.context().cast()) };
        f(env, &activity)
    })
    .map_err(java_error)
}

/// A readable message; Java exceptions arrive already caught and cleared.
fn java_error(e: jni::errors::Error) -> String {
    match e {
        jni::errors::Error::CaughtJavaException { name, msg, .. } => format!("{name}: {msg}"),
        other => other.to_string(),
    }
}

/// Loads one of our Kotlin classes (binary name, e.g. "app.kabar.OcrBridge")
/// with the Activity's class loader.
fn app_class<'local>(
    env: &mut Env<'local>,
    activity: &JObject,
    name: &JNIStr,
) -> jni::errors::Result<JClass<'local>> {
    let loader = env
        .call_method(activity, jni_str!("getClassLoader"), jni_sig!("()Ljava/lang/ClassLoader;"), &[])?
        .l()?;
    let loader = env.cast_local::<JClassLoader>(loader)?;
    LoaderContext::Loader(&loader).load_class(env, name, true)
}

fn to_string(env: &mut Env, obj: JObject) -> jni::errors::Result<Option<String>> {
    if obj.is_null() {
        return Ok(None);
    }
    let s = env.cast_local::<JString>(obj)?;
    Ok(Some(s.mutf8_chars(env)?.to_string()))
}

/// `Build.MANUFACTURER + " " + Build.MODEL`, e.g. "samsung SM-S928B".
pub fn device_model() -> Result<String, String> {
    with_activity(|env, _| {
        let mut field = |name| -> jni::errors::Result<String> {
            let v =
                env.get_static_field(jni_str!("android/os/Build"), name, jni_sig!("Ljava/lang/String;"))?;
            let s = env.cast_local::<JString>(v.l()?)?;
            Ok(s.mutf8_chars(env)?.to_string())
        };
        let maker = field(jni_str!("MANUFACTURER"))?;
        let model = field(jni_str!("MODEL"))?;
        Ok(format!("{maker} {model}"))
    })
}

/// `Context.getFilesDir().getAbsolutePath()`: private storage for the database.
pub fn files_dir() -> Result<String, String> {
    with_activity(|env, activity| {
        let dir =
            env.call_method(activity, jni_str!("getFilesDir"), jni_sig!("()Ljava/io/File;"), &[])?.l()?;
        let path =
            env.call_method(&dir, jni_str!("getAbsolutePath"), jni_sig!("()Ljava/lang/String;"), &[])?.l()?;
        let path = env.cast_local::<JString>(path)?;
        Ok(path.mutf8_chars(env)?.to_string())
    })
}

/// `PendingShare.take(activity)`: the JSON `ShareActivity` stored, parsed.
pub fn take_shared_input() -> Result<Option<SharedInput>, String> {
    let json = with_activity(|env, activity| {
        let class = app_class(env, activity, jni_str!("app.kabar.PendingShare"))?;
        let v = env
            .call_static_method(
                &class,
                jni_str!("take"),
                jni_sig!("(Landroid/content/Context;)Ljava/lang/String;"),
                &[JValue::Object(activity)],
            )?
            .l()?;
        to_string(env, v)
    })?;
    json.map(|j| serde_json::from_str(&j).map_err(|e| format!("unexpected shared item {j}: {e}"))).transpose()
}

/// `OcrBridge.recognize(path)`: ML Kit Text Recognition v2, blocking.
pub fn ocr_image(path: &Path) -> Result<OcrResult, String> {
    let path = path.to_str().ok_or("image path is not valid UTF-8")?;
    let json = with_activity(|env, activity| {
        let class = app_class(env, activity, jni_str!("app.kabar.OcrBridge"))?;
        let jpath = env.new_string(path)?;
        let v = env
            .call_static_method(
                &class,
                jni_str!("recognize"),
                jni_sig!("(Ljava/lang/String;)Ljava/lang/String;"),
                &[JValue::Object(&jpath)],
            )?
            .l()?;
        to_string(env, v)
    })?
    .ok_or("OCR returned nothing")?;
    super::ocr::parse_result_json(&json)
}

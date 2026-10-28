//! Android bridge: reaches Java/Kotlin through JNI from Rust, using the
//! JavaVM and Activity that tao/wry register in `ndk_context`.
//!
//! Framework classes (device info, storage, text-to-speech) are found directly. Our own Kotlin classes (`android/`,
//! package `app.kabar`) are loaded through the Activity's class loader, because
//! `FindClass` from a native thread only sees the system class loader.

#![allow(unsafe_code)]

use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use jni::objects::{JClass, JClassLoader, JObject, JString};
use jni::refs::{Global, LoaderContext};
use jni::strings::JNIStr;
use jni::{Env, JValue, JavaVM, jni_sig, jni_str};
use kabar_protocol::reader::{OcrResult, SharedInput};

use super::{SpeakError, TtsStatus};

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

/// `Context.getExternalFilesDir(null).getAbsolutePath()`: app storage a
/// computer can reach over USB (`Android/data/app.kabar/files`). Errors when
/// shared storage isn't mounted.
pub fn external_files_dir() -> Result<String, String> {
    let path = with_activity(|env, activity| {
        let dir = env
            .call_method(
                activity,
                jni_str!("getExternalFilesDir"),
                jni_sig!("(Ljava/lang/String;)Ljava/io/File;"),
                &[JValue::Object(&JObject::null())],
            )?
            .l()?;
        if dir.is_null() {
            return Ok(None);
        }
        let path =
            env.call_method(&dir, jni_str!("getAbsolutePath"), jni_sig!("()Ljava/lang/String;"), &[])?.l()?;
        to_string(env, path)
    })?;
    path.ok_or_else(|| "external storage is not available".to_string())
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

/// `ImagePick.open(activity)`: shows the system photo picker over Kabar.
pub fn pick_image() -> Result<(), String> {
    with_activity(|env, activity| {
        let class = app_class(env, activity, jni_str!("app.kabar.ImagePick"))?;
        env.call_static_method(
            &class,
            jni_str!("open"),
            jni_sig!("(Landroid/app/Activity;)V"),
            &[JValue::Object(activity)],
        )?;
        Ok(())
    })
}

/// `ImagePick.state()`; a FAILED state is reset once read.
pub fn pick_state() -> Result<super::PickState, String> {
    with_activity(|env, activity| {
        let class = app_class(env, activity, jni_str!("app.kabar.ImagePick"))?;
        let state = env.call_static_method(&class, jni_str!("state"), jni_sig!("()I"), &[])?.i()?;
        Ok(match state {
            1 => super::PickState::Open,
            2 => {
                env.call_static_method(&class, jni_str!("reset"), jni_sig!("()V"), &[])?;
                super::PickState::Failed
            }
            _ => super::PickState::Done,
        })
    })
}

/// `ExportSave.open(activity, path, name)`: the system "Save to" dialog for
/// the file at `path`.
pub fn save_document(path: &str, name: &str) -> Result<(), String> {
    with_activity(|env, activity| {
        let class = app_class(env, activity, jni_str!("app.kabar.ExportSave"))?;
        let (path, name) = (env.new_string(path)?, env.new_string(name)?);
        env.call_static_method(
            &class,
            jni_str!("open"),
            jni_sig!("(Landroid/app/Activity;Ljava/lang/String;Ljava/lang/String;)V"),
            &[JValue::Object(activity), JValue::Object(&path), JValue::Object(&name)],
        )?;
        Ok(())
    })
}

/// `ExportSave.state()` (and `name()` once saved); a finished state is reset.
pub fn save_state() -> Result<super::SaveState, String> {
    with_activity(|env, activity| {
        let class = app_class(env, activity, jni_str!("app.kabar.ExportSave"))?;
        let state = env.call_static_method(&class, jni_str!("state"), jni_sig!("()I"), &[])?.i()?;
        let out = match state {
            1 => return Ok(super::SaveState::Open),
            2 => super::SaveState::Failed,
            3 => {
                let name = env
                    .call_static_method(&class, jni_str!("name"), jni_sig!("()Ljava/lang/String;"), &[])?
                    .l()?;
                super::SaveState::Saved(to_string(env, name)?.filter(|n| !n.is_empty()))
            }
            _ => super::SaveState::Cancelled,
        };
        env.call_static_method(&class, jni_str!("reset"), jni_sig!("()V"), &[])?;
        Ok(out)
    })
}

/// What `UpdateInstaller` is doing (Settings › About › Updates).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallState {
    Idle,
    Working,
    /// Android's confirmation screen is up.
    WaitingUser,
    Failed(String),
    Cancelled,
}

/// `UpdateInstaller.canRequestInstalls(activity)`: "Install unknown apps" is on.
pub fn can_request_installs() -> Result<bool, String> {
    with_activity(|env, activity| {
        let class = app_class(env, activity, jni_str!("app.kabar.UpdateInstaller"))?;
        env.call_static_method(
            &class,
            jni_str!("canRequestInstalls"),
            jni_sig!("(Landroid/app/Activity;)Z"),
            &[JValue::Object(activity)],
        )?
        .z()
    })
}

/// `UpdateInstaller.canInstallQuietly(activity)`: Android 12+ and Kabar
/// installed itself last time, so no confirmation screen.
pub fn can_install_quietly() -> Result<bool, String> {
    with_activity(|env, activity| {
        let class = app_class(env, activity, jni_str!("app.kabar.UpdateInstaller"))?;
        env.call_static_method(
            &class,
            jni_str!("canInstallQuietly"),
            jni_sig!("(Landroid/app/Activity;)Z"),
            &[JValue::Object(activity)],
        )?
        .z()
    })
}

/// `UpdateInstaller.openInstallPermission(activity)`.
pub fn open_install_permission() -> Result<(), String> {
    with_activity(|env, activity| {
        let class = app_class(env, activity, jni_str!("app.kabar.UpdateInstaller"))?;
        env.call_static_method(
            &class,
            jni_str!("openInstallPermission"),
            jni_sig!("(Landroid/app/Activity;)V"),
            &[JValue::Object(activity)],
        )?;
        Ok(())
    })
}

/// `UpdateInstaller.install(...)`: checks the APK (package, signing key,
/// versionCode) and hands it to PackageInstaller. Returns at once; poll
/// [`install_state`].
pub fn install_apk(
    path: &str,
    version_code: u64,
    done_title: &str,
    done_body: &str,
    ready_title: &str,
) -> Result<(), String> {
    with_activity(|env, activity| {
        let class = app_class(env, activity, jni_str!("app.kabar.UpdateInstaller"))?;
        let (path, title, body) =
            (env.new_string(path)?, env.new_string(done_title)?, env.new_string(done_body)?);
        let ready = env.new_string(ready_title)?;
        env.call_static_method(
            &class,
            jni_str!("install"),
            jni_sig!(
                "(Landroid/app/Activity;Ljava/lang/String;JLjava/lang/String;Ljava/lang/String;Ljava/lang/String;)V"
            ),
            &[
                JValue::Object(activity),
                JValue::Object(&path),
                JValue::Long(i64::try_from(version_code).unwrap_or(0)),
                JValue::Object(&title),
                JValue::Object(&body),
                JValue::Object(&ready),
            ],
        )?;
        Ok(())
    })
}

/// `UpdateInstaller.isMetered(activity)`: mobile data or Data Saver.
pub fn network_metered() -> Result<bool, String> {
    with_activity(|env, activity| {
        let class = app_class(env, activity, jni_str!("app.kabar.UpdateInstaller"))?;
        env.call_static_method(
            &class,
            jni_str!("isMetered"),
            jni_sig!("(Landroid/app/Activity;)Z"),
            &[JValue::Object(activity)],
        )?
        .z()
    })
}

/// `SystemBars.isNight(activity)`: the phone's own dark mode.
pub fn is_night() -> Result<bool, String> {
    with_activity(|env, activity| {
        let class = app_class(env, activity, jni_str!("app.kabar.SystemBars"))?;
        env.call_static_method(
            &class,
            jni_str!("isNight"),
            jni_sig!("(Landroid/app/Activity;)Z"),
            &[JValue::Object(activity)],
        )?
        .z()
    })
}

/// `SystemBars.apply(activity, day, night)`: bar colours as ARGB ints.
pub fn system_bars(day: i32, night: i32) -> Result<(), String> {
    with_activity(|env, activity| {
        let class = app_class(env, activity, jni_str!("app.kabar.SystemBars"))?;
        env.call_static_method(
            &class,
            jni_str!("apply"),
            jni_sig!("(Landroid/app/Activity;II)V"),
            &[JValue::Object(activity), JValue::Int(day), JValue::Int(night)],
        )?;
        Ok(())
    })
}

/// `Haptics.available(activity)`: the device has a vibration motor.
pub fn has_vibrator() -> Result<bool, String> {
    with_activity(|env, activity| {
        let class = app_class(env, activity, jni_str!("app.kabar.Haptics"))?;
        env.call_static_method(
            &class,
            jni_str!("available"),
            jni_sig!("(Landroid/content/Context;)Z"),
            &[JValue::Object(activity)],
        )?
        .z()
    })
}

/// `Haptics.play(activity, kind)`.
pub fn vibrate(kind: i32) -> Result<(), String> {
    with_activity(|env, activity| {
        let class = app_class(env, activity, jni_str!("app.kabar.Haptics"))?;
        env.call_static_method(
            &class,
            jni_str!("play"),
            jni_sig!("(Landroid/content/Context;I)V"),
            &[JValue::Object(activity), JValue::Int(kind)],
        )?;
        Ok(())
    })
}

/// `UpdateInstaller.showPendingConfirm(activity)`: the confirmation screen
/// that arrived while Kabar was in the background.
pub fn show_pending_confirm() -> Result<(), String> {
    with_activity(|env, activity| {
        let class = app_class(env, activity, jni_str!("app.kabar.UpdateInstaller"))?;
        env.call_static_method(
            &class,
            jni_str!("showPendingConfirm"),
            jni_sig!("(Landroid/app/Activity;)V"),
            &[JValue::Object(activity)],
        )?;
        Ok(())
    })
}

/// `UpdateInstaller.state()`; a finished state is reset once read.
pub fn install_state() -> Result<InstallState, String> {
    with_activity(|env, activity| {
        let class = app_class(env, activity, jni_str!("app.kabar.UpdateInstaller"))?;
        let state = env.call_static_method(&class, jni_str!("state"), jni_sig!("()I"), &[])?.i()?;
        let out = match state {
            1 => return Ok(InstallState::Working),
            2 => return Ok(InstallState::WaitingUser),
            3 => {
                let msg = env
                    .call_static_method(&class, jni_str!("message"), jni_sig!("()Ljava/lang/String;"), &[])?
                    .l()?;
                InstallState::Failed(to_string(env, msg)?.unwrap_or_default())
            }
            4 => InstallState::Cancelled,
            _ => InstallState::Idle,
        };
        env.call_static_method(&class, jni_str!("reset"), jni_sig!("()V"), &[])?;
        Ok(out)
    })
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

/// `Settings.Global.DEVICE_NAME`: the name the user sees in Settings → About
/// phone, e.g. "Galaxy S24 Ultra".
pub fn device_name() -> Result<Option<String>, String> {
    with_activity(|env, activity| {
        let resolver = env
            .call_method(
                activity,
                jni_str!("getContentResolver"),
                jni_sig!("()Landroid/content/ContentResolver;"),
                &[],
            )?
            .l()?;
        let key = env.new_string("device_name")?;
        let value = env
            .call_static_method(
                jni_str!("android/provider/Settings$Global"),
                jni_str!("getString"),
                jni_sig!("(Landroid/content/ContentResolver;Ljava/lang/String;)Ljava/lang/String;"),
                &[(&resolver).into(), (&key).into()],
            )?
            .l()?;
        if value.is_null() {
            return Ok(None);
        }
        let value = env.cast_local::<JString>(value)?;
        Ok(Some(value.mutf8_chars(env)?.to_string()).filter(|s| !s.is_empty()))
    })
}

/// Opens `url` in the browser with an `ACTION_VIEW` intent.
pub fn open_url(url: &str) -> Result<(), String> {
    with_activity(|env, activity| {
        let action = env
            .get_static_field(
                jni_str!("android/content/Intent"),
                jni_str!("ACTION_VIEW"),
                jni_sig!("Ljava/lang/String;"),
            )?
            .l()?;
        let url = env.new_string(url)?;
        let uri = env
            .call_static_method(
                jni_str!("android/net/Uri"),
                jni_str!("parse"),
                jni_sig!("(Ljava/lang/String;)Landroid/net/Uri;"),
                &[(&url).into()],
            )?
            .l()?;
        let intent = env.new_object(
            jni_str!("android/content/Intent"),
            jni_sig!("(Ljava/lang/String;Landroid/net/Uri;)V"),
            &[(&action).into(), (&uri).into()],
        )?;
        // FLAG_ACTIVITY_NEW_TASK
        env.call_method(
            &intent,
            jni_str!("addFlags"),
            jni_sig!("(I)Landroid/content/Intent;"),
            &[0x1000_0000.into()],
        )?;
        env.call_method(
            activity,
            jni_str!("startActivity"),
            jni_sig!("(Landroid/content/Intent;)V"),
            &[(&intent).into()],
        )?;
        Ok(())
    })
}

/// `NotificationManager.areNotificationsEnabled()` for Kabar.
pub fn notifications_enabled() -> Result<bool, String> {
    with_activity(|env, activity| {
        let name = env.new_string("notification")?;
        let manager = env
            .call_method(
                activity,
                jni_str!("getSystemService"),
                jni_sig!("(Ljava/lang/String;)Ljava/lang/Object;"),
                &[(&name).into()],
            )?
            .l()?;
        env.call_method(&manager, jni_str!("areNotificationsEnabled"), jni_sig!("()Z"), &[])?.z()
    })
}

/// Opens Kabar's own page in Android Settings: its notification settings, or
/// App info (storage, battery, permissions).
pub fn open_app_page(page: super::AppPage) -> Result<(), String> {
    with_activity(|env, activity| {
        let package = env
            .call_method(activity, jni_str!("getPackageName"), jni_sig!("()Ljava/lang/String;"), &[])?
            .l()?;
        let intent = match page {
            super::AppPage::Notifications => {
                let action = env.new_string("android.settings.APP_NOTIFICATION_SETTINGS")?;
                let intent = env.new_object(
                    jni_str!("android/content/Intent"),
                    jni_sig!("(Ljava/lang/String;)V"),
                    &[(&action).into()],
                )?;
                let extra = env.new_string("android.provider.extra.APP_PACKAGE")?;
                env.call_method(
                    &intent,
                    jni_str!("putExtra"),
                    jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;"),
                    &[(&extra).into(), (&package).into()],
                )?;
                intent
            }
            super::AppPage::Details => {
                let action = env.new_string("android.settings.APPLICATION_DETAILS_SETTINGS")?;
                let package = to_string(env, package)?.unwrap_or_default();
                let uri = env.new_string(format!("package:{package}"))?;
                let uri = env
                    .call_static_method(
                        jni_str!("android/net/Uri"),
                        jni_str!("parse"),
                        jni_sig!("(Ljava/lang/String;)Landroid/net/Uri;"),
                        &[(&uri).into()],
                    )?
                    .l()?;
                env.new_object(
                    jni_str!("android/content/Intent"),
                    jni_sig!("(Ljava/lang/String;Landroid/net/Uri;)V"),
                    &[(&action).into(), (&uri).into()],
                )?
            }
        };
        // FLAG_ACTIVITY_NEW_TASK
        env.call_method(
            &intent,
            jni_str!("addFlags"),
            jni_sig!("(I)Landroid/content/Intent;"),
            &[0x1000_0000.into()],
        )?;
        env.call_method(
            activity,
            jni_str!("startActivity"),
            jni_sig!("(Landroid/content/Intent;)V"),
            &[(&intent).into()],
        )?;
        Ok(())
    })
}

// ----------------------------------------------------------- background audio
//
// Downloaded episodes play in `PlaybackService` (android/…/PlaybackService.kt):
// a foreground media service with a MediaSession, so they keep playing with
// the screen off and show lock-screen controls. Rust drives it through the
// static `PlaybackBridge` methods and polls its position.

fn playback<T>(f: impl FnOnce(&mut Env, &JClass, &JObject) -> jni::errors::Result<T>) -> Result<T, String> {
    with_activity(|env, activity| {
        let class = app_class(env, activity, jni_str!("app.kabar.PlaybackBridge"))?;
        f(env, &class, activity)
    })
}

/// Loads the MP3 at `path` (if it isn't already) and plays it from `at` seconds.
pub fn track_play(path: &str, title: &str, show: &str, at: f64, rate: f32) -> Result<(), String> {
    playback(|env, class, activity| {
        let (path, title, show) = (env.new_string(path)?, env.new_string(title)?, env.new_string(show)?);
        env.call_static_method(
            class,
            jni_str!("play"),
            jni_sig!("(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;DF)V"),
            &[
                JValue::Object(activity),
                JValue::Object(&path),
                JValue::Object(&title),
                JValue::Object(&show),
                JValue::Double(at),
                JValue::Float(rate),
            ],
        )?;
        Ok(())
    })
}

pub fn track_pause() -> Result<(), String> {
    playback(|env, class, _| env.call_static_method(class, jni_str!("pause"), jni_sig!("()V"), &[]).map(drop))
}

pub fn track_resume() -> Result<(), String> {
    playback(|env, class, _| {
        env.call_static_method(class, jni_str!("resume"), jni_sig!("()V"), &[]).map(drop)
    })
}

pub fn track_seek(at: f64) -> Result<(), String> {
    playback(|env, class, _| {
        env.call_static_method(class, jni_str!("seek"), jni_sig!("(D)V"), &[JValue::Double(at)]).map(drop)
    })
}

pub fn track_rate(rate: f32) -> Result<(), String> {
    playback(|env, class, _| {
        env.call_static_method(class, jni_str!("setRate"), jni_sig!("(F)V"), &[JValue::Float(rate)]).map(drop)
    })
}

/// Stops playback and the service.
pub fn track_stop() -> Result<(), String> {
    playback(|env, class, activity| {
        env.call_static_method(
            class,
            jni_str!("stop"),
            jni_sig!("(Landroid/content/Context;)V"),
            &[JValue::Object(activity)],
        )
        .map(drop)
    })
}

/// (seconds into the track or -1 when nothing is loaded, playing, ended).
pub fn track_state() -> Result<(f64, bool, bool), String> {
    playback(|env, class, _| {
        let at = env.call_static_method(class, jni_str!("position"), jni_sig!("()D"), &[])?.d()?;
        let playing = env.call_static_method(class, jni_str!("isPlaying"), jni_sig!("()Z"), &[])?.z()?;
        let ended = env.call_static_method(class, jni_str!("hasEnded"), jni_sig!("()Z"), &[])?.z()?;
        Ok((at, playing, ended))
    })
}

// ---------------------------------------------------------------- text-to-speech
//
// Android's System WebView has no `speechSynthesis`, so lesson and review
// audio goes to the framework `android.speech.tts.TextToSpeech` instead. One
// engine per process, created lazily with a null OnInitListener; it binds to
// the TTS service asynchronously, so readiness is polled (`getVoices()` is
// null until the service is connected) from a short-lived background thread
// and the UI thread never waits on it. No unsafe code below: every call goes
// through `with_activity`.

/// How long a fresh engine gets to connect before we call it unavailable.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
/// After a failed start, wait this long before trying a new engine.
const RETRY_AFTER: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_millis(100);

/// `TextToSpeech.QUEUE_FLUSH`, `ERROR`, `LANG_MISSING_DATA`, `LANG_NOT_SUPPORTED`.
const QUEUE_FLUSH: i32 = 0;
const ERROR: i32 = -1;
const LANG_MISSING_DATA: i32 = -1;
const LANG_NOT_SUPPORTED: i32 = -2;

struct Utterance {
    text: String,
    lang_tag: String,
    rate: f32,
}

struct Tts {
    engine: Option<Global<JObject<'static>>>,
    created: Option<Instant>,
    failed: Option<Instant>,
    pending: Option<Utterance>,
    waiting: bool,
    status: TtsStatus,
}

static TTS: Mutex<Tts> = Mutex::new(Tts {
    engine: None,
    created: None,
    failed: None,
    pending: None,
    waiting: false,
    status: TtsStatus::Starting,
});

fn tts() -> MutexGuard<'static, Tts> {
    TTS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Last thing the engine told us, for the "no audio" hint.
pub fn tts_status() -> TtsStatus {
    tts().status
}

/// Speaks `text` with the system voice for `lang_tag` ("id-ID" or "jv-ID").
/// `Ok` means it was spoken, or queued while the engine is still starting.
pub fn speak(text: &str, lang_tag: &str, rate: f32) -> Result<(), SpeakError> {
    let mut guard = tts();
    let t = &mut *guard;
    if t.engine.is_none() {
        if t.failed.is_some_and(|at| at.elapsed() < RETRY_AFTER) {
            return Err(SpeakError::NoEngine);
        }
        match create_engine() {
            Ok(engine) => {
                t.engine = Some(engine);
                t.created = Some(Instant::now());
                t.failed = None;
                t.status = TtsStatus::Starting;
            }
            Err(e) => {
                crate::diag::report!("TextToSpeech could not start: {e}");
                t.failed = Some(Instant::now());
                t.status = TtsStatus::NoEngine;
                return Err(SpeakError::NoEngine);
            }
        }
    }
    let utterance = Utterance { text: text.to_owned(), lang_tag: lang_tag.to_owned(), rate };
    let Some(engine) = t.engine.as_ref().map(|g| g.as_obj()) else {
        return Err(SpeakError::NoEngine);
    };
    match is_ready(engine) {
        Ok(true) => {
            let result = say(engine, &utterance);
            t.status = status_of(&result);
            result
        }
        Ok(false) => {
            // Still binding to the service: keep only the latest request and
            // let a background thread speak it once the engine is up.
            t.pending = Some(utterance);
            if !t.waiting {
                t.waiting = true;
                std::thread::spawn(wait_until_ready);
            }
            Ok(())
        }
        Err(e) => Err(SpeakError::Failed(e)),
    }
}

fn wait_until_ready() {
    loop {
        std::thread::sleep(POLL);
        let mut guard = tts();
        let t = &mut *guard;
        let Some(engine) = t.engine.as_ref().map(|g| g.as_obj()) else {
            t.waiting = false;
            return;
        };
        if is_ready(engine).unwrap_or(false) {
            let result = match t.pending.take() {
                Some(u) => say(engine, &u),
                None => Ok(()),
            };
            if let Err(e) = &result {
                crate::diag::report!("TextToSpeech: {e:?}");
            }
            t.status = status_of(&result);
            t.waiting = false;
            return;
        }
        if t.created.is_none_or(|at| at.elapsed() > CONNECT_TIMEOUT) {
            // No engine ever answered (none installed or all disabled).
            // Release this one and allow a fresh try later.
            crate::diag::report!("TextToSpeech did not connect within {CONNECT_TIMEOUT:?}");
            if let Some(engine) = t.engine.take() {
                let _ = with_activity(|env, _| {
                    env.call_method(engine.as_obj(), jni_str!("shutdown"), jni_sig!("()V"), &[])?;
                    Ok(())
                });
            }
            t.pending = None;
            t.failed = Some(Instant::now());
            t.status = TtsStatus::NoEngine;
            t.waiting = false;
            return;
        }
    }
}

fn status_of(result: &Result<(), SpeakError>) -> TtsStatus {
    match result {
        Ok(()) | Err(SpeakError::Failed(_) | SpeakError::UseWebView) => TtsStatus::Ready,
        Err(SpeakError::MissingVoice) => TtsStatus::MissingVoice,
        Err(SpeakError::Unsupported) => TtsStatus::Unsupported,
        Err(SpeakError::NoEngine) => TtsStatus::NoEngine,
    }
}

/// `new TextToSpeech(activity.getApplicationContext(), null)`, held globally.
fn create_engine() -> Result<Global<JObject<'static>>, String> {
    with_activity(|env, activity| {
        let app = env
            .call_method(
                activity,
                jni_str!("getApplicationContext"),
                jni_sig!("()Landroid/content/Context;"),
                &[],
            )?
            .l()?;
        let listener = JObject::null();
        let engine = env.new_object(
            jni_str!("android/speech/tts/TextToSpeech"),
            jni_sig!("(Landroid/content/Context;Landroid/speech/tts/TextToSpeech$OnInitListener;)V"),
            &[JValue::Object(&app), JValue::Object(&listener)],
        )?;
        env.new_global_ref(engine)
    })
}

/// The service is connected once `getVoices()` stops returning null.
fn is_ready(engine: &JObject<'static>) -> Result<bool, String> {
    with_activity(|env, _| {
        let voices =
            env.call_method(engine, jni_str!("getVoices"), jni_sig!("()Ljava/util/Set;"), &[])?.l()?;
        Ok(!voices.is_null())
    })
}

/// `setLanguage(new Locale(language, country))`, returning a `LANG_*` code.
fn set_language(
    env: &mut Env,
    engine: &JObject<'static>,
    language: &str,
    country: &str,
) -> jni::errors::Result<i32> {
    let language = env.new_string(language)?;
    let country = env.new_string(country)?;
    let locale = env.new_object(
        jni_str!("java/util/Locale"),
        jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
        &[JValue::Object(&language), JValue::Object(&country)],
    )?;
    env.call_method(
        engine,
        jni_str!("setLanguage"),
        jni_sig!("(Ljava/util/Locale;)I"),
        &[JValue::Object(&locale)],
    )?
    .i()
}

/// Picks the voice and speaks. Javanese asks for jv-ID and falls back to the
/// Indonesian voice, which reads Javanese passably.
fn say(engine: &JObject<'static>, u: &Utterance) -> Result<(), SpeakError> {
    let outcome = with_activity(|env, _| {
        let mut code = LANG_NOT_SUPPORTED;
        if u.lang_tag.starts_with("jv") {
            code = set_language(env, engine, "jv", "ID")?;
        }
        if code < 0 {
            code = set_language(env, engine, "id", "ID")?;
        }
        if code < 0 {
            return Ok(Err(code));
        }
        env.call_method(engine, jni_str!("setSpeechRate"), jni_sig!("(F)I"), &[JValue::Float(u.rate)])?;
        let text = env.new_string(&u.text)?;
        let id = env.new_string("kabar")?;
        let bundle = JObject::null();
        let spoken = env
            .call_method(
                engine,
                jni_str!("speak"),
                jni_sig!("(Ljava/lang/CharSequence;ILandroid/os/Bundle;Ljava/lang/String;)I"),
                &[
                    JValue::Object(&text),
                    JValue::Int(QUEUE_FLUSH),
                    JValue::Object(&bundle),
                    JValue::Object(&id),
                ],
            )?
            .i()?;
        Ok(Ok(spoken))
    })
    .map_err(SpeakError::Failed)?;
    match outcome {
        Ok(ERROR) => Err(SpeakError::Failed("TextToSpeech.speak returned ERROR".into())),
        Ok(_) => Ok(()),
        Err(LANG_MISSING_DATA) => Err(SpeakError::MissingVoice),
        Err(_) => Err(SpeakError::Unsupported),
    }
}

// ------------------------------------------------------------ reminders
//
// Kak Nur's local reminders live in `ReminderBridge` (android/…/Reminders.kt):
// Rust sends the next few as JSON and Android's AlarmManager posts them.

fn reminders<T>(f: impl FnOnce(&mut Env, &JClass, &JObject) -> jni::errors::Result<T>) -> Result<T, String> {
    with_activity(|env, activity| {
        let class = app_class(env, activity, jni_str!("app.kabar.ReminderBridge"))?;
        f(env, &class, activity)
    })
}

/// Replaces the scheduled reminders with `json` (`[{at, title, text}]`).
pub fn schedule_reminders(json: &str) -> Result<(), String> {
    reminders(|env, class, activity| {
        let json = env.new_string(json)?;
        env.call_static_method(
            class,
            jni_str!("schedule"),
            jni_sig!("(Landroid/content/Context;Ljava/lang/String;)V"),
            &[JValue::Object(activity), JValue::Object(&json)],
        )
        .map(drop)
    })
}

/// Shows Android 13+'s notification prompt if Kabar hasn't been allowed yet.
pub fn request_notifications() -> Result<(), String> {
    reminders(|env, class, activity| {
        env.call_static_method(
            class,
            jni_str!("requestPermission"),
            jni_sig!("(Landroid/app/Activity;)V"),
            &[JValue::Object(activity)],
        )
        .map(drop)
    })
}

/// The reminders Android showed since the last call, as unix ms.
pub fn take_delivered_reminders() -> Result<Vec<u64>, String> {
    let json = reminders(|env, class, activity| {
        let s = env
            .call_static_method(
                class,
                jni_str!("takeDelivered"),
                jni_sig!("(Landroid/content/Context;)Ljava/lang/String;"),
                &[JValue::Object(activity)],
            )?
            .l()?;
        to_string(env, s)
    })?;
    Ok(json.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default())
}

/// Whether Kabar is exempt from battery optimisation (Samsung's sleeping apps).
pub fn battery_unrestricted() -> Result<bool, String> {
    reminders(|env, class, activity| {
        env.call_static_method(
            class,
            jni_str!("batteryUnrestricted"),
            jni_sig!("(Landroid/content/Context;)Z"),
            &[JValue::Object(activity)],
        )?
        .z()
    })
}

// ------------------------------------------------------------ update notice
//
// "A new Kabar is out": `UpdateNotice` (android/…/UpdateNotice.kt) posts it
// and schedules `UpdateCheckJob`, which calls back into Rust below while
// Kabar may be closed.

fn update_notice<T>(
    f: impl FnOnce(&mut Env, &JClass, &JObject) -> jni::errors::Result<T>,
) -> Result<T, String> {
    with_activity(|env, activity| {
        let class = app_class(env, activity, jni_str!("app.kabar.UpdateNotice"))?;
        f(env, &class, activity)
    })
}

/// Posts the notification unless Kabar is in front or notifications are off.
pub fn post_update_notice(n: &crate::update::Notice) -> Result<bool, String> {
    let json = serde_json::to_string(n).map_err(|e| e.to_string())?;
    update_notice(|env, class, activity| {
        let json = env.new_string(&json)?;
        env.call_static_method(
            class,
            jni_str!("show"),
            jni_sig!("(Landroid/content/Context;Ljava/lang/String;Z)Z"),
            &[JValue::Object(activity), JValue::Object(&json), JValue::Bool(true)],
        )?
        .z()
    })
}

pub fn clear_update_notice() -> Result<(), String> {
    update_notice(|env, class, activity| {
        env.call_static_method(
            class,
            jni_str!("clear"),
            jni_sig!("(Landroid/content/Context;)V"),
            &[JValue::Object(activity)],
        )
        .map(drop)
    })
}

/// Starts or stops the periodic background check (JobScheduler).
pub fn schedule_update_checks(on: bool) -> Result<(), String> {
    update_notice(|env, class, activity| {
        env.call_static_method(
            class,
            jni_str!("scheduleChecks"),
            jni_sig!("(Landroid/content/Context;Z)V"),
            &[JValue::Object(activity), JValue::Bool(on)],
        )
        .map(drop)
    })
}

/// The action a notification tap left for the app, once.
pub fn take_update_tap() -> Result<Option<String>, String> {
    update_notice(|env, class, activity| {
        let s = env
            .call_static_method(
                class,
                jni_str!("takeTap"),
                jni_sig!("(Landroid/content/Context;)Ljava/lang/String;"),
                &[JValue::Object(activity)],
            )?
            .l()?;
        to_string(env, s)
    })
}

/// `UpdateCheckJob.check(dataDir, metered, canNotify, localMinute)`: the
/// background check, from a JobScheduler thread in a process that may have
/// no Activity (so nothing here may use `with_activity`). Returns the
/// notification's JSON, or null.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_kabar_UpdateCheckJob_check<'caller>(
    mut unowned_env: jni::EnvUnowned<'caller>,
    _this: JObject<'caller>,
    data_dir: JString<'caller>,
    metered: jni::sys::jboolean,
    can_notify: jni::sys::jboolean,
    local_minute: jni::sys::jint,
) -> JString<'caller> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JString<'caller>> {
            let dir = data_dir.mutf8_chars(env)?.to_string();
            let minute = u16::try_from(local_minute.clamp(0, 24 * 60 - 1)).unwrap_or(0);
            let local = kabar_protocol::coach::LocalTime::hm(minute / 60, minute % 60);
            let notice = crate::update::background_check(Path::new(&dir), metered, can_notify, local);
            match notice.and_then(|n| serde_json::to_string(&n).ok()) {
                Some(json) => JString::from_str(env, json),
                None => Ok(JString::default()),
            }
        })
        .resolve::<jni::errors::LogErrorAndDefault>()
}

/// `UpdateCheckJob.shown(dataDir, key)`: Android showed the notification
/// `check` returned, so it isn't sent again (one that failed to show is
/// tried again at the next run).
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_kabar_UpdateCheckJob_shown<'caller>(
    mut unowned_env: jni::EnvUnowned<'caller>,
    _this: JObject<'caller>,
    data_dir: JString<'caller>,
    key: JString<'caller>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let dir = data_dir.mutf8_chars(env)?.to_string();
            let key = key.mutf8_chars(env)?.to_string();
            crate::update::background_shown(Path::new(&dir), &key);
            Ok(())
        })
        .resolve::<jni::errors::LogErrorAndDefault>()
}

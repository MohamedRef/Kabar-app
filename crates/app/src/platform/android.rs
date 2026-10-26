//! Android bridge spike: reaches Java through JNI from Rust, using the
//! JavaVM and Activity that tao/wry register in `ndk_context`.
//!
//! Framework classes only so far: device info, storage, text-to-speech.
//! M1 proved the path. Our own Kotlin classes
//! (sign-in, FCM, ML Kit) need the Activity's class loader; that lands in M2.

#![allow(unsafe_code)]

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use jni::objects::{JObject, JString};
use jni::refs::Global;
use jni::{Env, JValue, JavaVM, jni_sig, jni_str};

use super::{SpeakError, TtsStatus};

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
    .map_err(|e: jni::errors::Error| e.to_string())
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
                eprintln!("kabar: TextToSpeech could not start: {e}");
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
                eprintln!("kabar: TextToSpeech: {e:?}");
            }
            t.status = status_of(&result);
            t.waiting = false;
            return;
        }
        if t.created.is_none_or(|at| at.elapsed() > CONNECT_TIMEOUT) {
            // No engine ever answered (none installed or all disabled).
            // Release this one and allow a fresh try later.
            eprintln!("kabar: TextToSpeech did not connect within {CONNECT_TIMEOUT:?}");
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

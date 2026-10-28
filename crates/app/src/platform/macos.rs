//! macOS bridge: on-device OCR with Apple Vision (`VNRecognizeTextRequest`,
//! accurate level), through the objc2 framework crates.

#![allow(unsafe_code)]

use std::path::Path;

use kabar_protocol::reader::{OcrEngine, OcrLine, OcrResult};
use objc2::AnyThread;
use objc2::rc::autoreleasepool;
use objc2_core_foundation::{CFNumber, CFString, CFType, CFURL};
use objc2_core_graphics::CGImage;
use objc2_foundation::{NSArray, NSDictionary, NSString};
use objc2_image_io::{CGImagePropertyOrientation, CGImageSource, kCGImagePropertyOrientation};
use objc2_vision::{VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel};

use super::ocr;

/// Preferred recognition languages, used when this macOS supports them.
const LANGUAGES: [&str; 2] = ["id-ID", "en-US"];

pub fn ocr_image(path: &Path) -> Result<OcrResult, String> {
    autoreleasepool(|_| recognize(path))
}

fn recognize(path: &Path) -> Result<OcrResult, String> {
    let url = CFURL::from_file_path(path).ok_or("image path is not a file path")?;
    // SAFETY: no options dictionary is passed.
    let source = unsafe { CGImageSource::with_url(&url, None) }.ok_or("could not open the image")?;
    // SAFETY: no options dictionary is passed; a missing index returns None.
    let image = unsafe { source.image_at_index(0, None) }.ok_or("not an image macOS can read")?;
    let orientation = exif_orientation(&source);
    let (raw_w, raw_h) = (CGImage::width(Some(&image)) as f64, CGImage::height(Some(&image)) as f64);
    // Vision reports boxes in the upright image; orientations 5-8 swap the sides.
    let (width, height) = if orientation as u32 >= 5 { (raw_h, raw_w) } else { (raw_w, raw_h) };

    let request = VNRecognizeTextRequest::new();
    request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
    let preferred = supported_languages(&request);
    // Language correction toward English would "fix" Indonesian and Javanese
    // words, so it is only on when Vision knows Indonesian.
    request.setUsesLanguageCorrection(preferred.iter().any(|l| l.starts_with("id")));
    if !preferred.is_empty() {
        let langs: Vec<_> = preferred.iter().map(|l| NSString::from_str(l)).collect();
        request.setRecognitionLanguages(&NSArray::from_retained_slice(&langs));
    }

    let options = NSDictionary::new();
    // SAFETY: `options` is an empty dictionary of the type Vision expects and
    // `image` is a valid CGImage that outlives the handler.
    let handler = unsafe {
        VNImageRequestHandler::initWithCGImage_orientation_options(
            VNImageRequestHandler::alloc(),
            &image,
            orientation,
            &options,
        )
    };
    let as_request: &VNRequest = &request;
    handler
        .performRequests_error(&NSArray::from_slice(&[as_request]))
        .map_err(|e| format!("Vision failed: {}", e.localizedDescription()))?;

    let mut lines = Vec::new();
    for observation in request.results().iter().flat_map(|r| r.iter()) {
        let Some(best) = observation.topCandidates(1).firstObject() else { continue };
        // SAFETY: plain struct getter on a valid observation.
        let b = unsafe { observation.boundingBox() };
        lines.push(OcrLine {
            text: best.string().to_string(),
            bbox: ocr::normalized_bottom_left_to_pixels(
                b.origin.x,
                b.origin.y,
                b.size.width,
                b.size.height,
                width,
                height,
            ),
            confidence: Some(best.confidence()),
        });
    }

    Ok(ocr::tidy(OcrResult {
        engine: OcrEngine::AppleVision,
        width: width as f32,
        height: height as f32,
        blocks: ocr::group_lines(lines),
    }))
}

/// The subset of [`LANGUAGES`] this macOS supports for the request, in order.
fn supported_languages(request: &VNRecognizeTextRequest) -> Vec<&'static str> {
    // SAFETY: a plain query on a configured request; errors are handled.
    let Ok(supported) = (unsafe { request.supportedRecognitionLanguagesAndReturnError() }) else {
        return Vec::new();
    };
    let supported: Vec<String> = supported.iter().map(|s| s.to_string()).collect();
    LANGUAGES
        .into_iter()
        .filter(|want| {
            let lang = want.split('-').next().unwrap_or(want);
            supported.iter().any(|s| s == want || s == lang || s.starts_with(&format!("{lang}-")))
        })
        .collect()
}

/// EXIF orientation of the first image, `Up` when absent.
fn exif_orientation(source: &CGImageSource) -> CGImagePropertyOrientation {
    use CGImagePropertyOrientation as O;
    // SAFETY: no options dictionary is passed.
    let Some(props) = (unsafe { source.properties_at_index(0, None) }) else { return O::Up };
    // SAFETY: ImageIO property dictionaries have CFString keys and CF values, and
    // `props` is our own copy, never mutated.
    let props = unsafe { props.cast_unchecked::<CFString, CFType>() };
    // SAFETY: an immutable CFString constant exported by ImageIO.
    let key = unsafe { kCGImagePropertyOrientation };
    let value = props.get(key).and_then(|v| v.downcast::<CFNumber>().ok()).and_then(|n| n.as_i32());
    match value {
        Some(2) => O::UpMirrored,
        Some(3) => O::Down,
        Some(4) => O::DownMirrored,
        Some(5) => O::LeftMirrored,
        Some(6) => O::Right,
        Some(7) => O::RightMirrored,
        Some(8) => O::Left,
        _ => O::Up,
    }
}

// ------------------------------------------------------------ reminders
//
// Kak Nur's reminders as native notifications, from the same plan Android
// gets (`coach::notify::schedule`). UNUserNotificationCenter needs a real app
// bundle (`app.kabar`): from `cargo run` there is none and the framework
// would abort, so everything here checks for one first and does nothing
// without it. Calendar triggers fire at the local date and time in the
// Mac's own zone, so daylight saving never moves a reminder.

pub mod reminders {
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::{NSBundle, NSDateComponents, NSError, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNCalendarNotificationTrigger, UNMutableNotificationContent,
        UNNotificationRequest, UNUserNotificationCenter,
    };
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Item {
        date: String,
        time: String,
        title: String,
        text: String,
    }

    /// True inside the bundled Kabar.app, where notifications can work.
    pub fn bundled() -> bool {
        NSBundle::mainBundle().bundleIdentifier().is_some()
    }

    /// Asks macOS once for permission to show Kak Nur's notifications.
    pub fn request() {
        if !bundled() {
            return;
        }
        let done = RcBlock::new(|granted: Bool, _error: *mut NSError| {
            if !granted.as_bool() {
                crate::diag::report!("notifications not allowed on this Mac");
            }
        });
        UNUserNotificationCenter::currentNotificationCenter()
            .requestAuthorizationWithOptions_completionHandler(
                UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
                &done,
            );
    }

    /// Replaces Kabar's pending notifications with `json`'s.
    pub fn schedule(json: &str) -> Result<(), String> {
        if !bundled() {
            return Ok(());
        }
        let items: Vec<Item> = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let center = UNUserNotificationCenter::currentNotificationCenter();
        center.removeAllPendingNotificationRequests();
        for (i, item) in items.iter().enumerate() {
            let (Some((y, mo, d)), Some((h, mi))) = (ymd(&item.date), hm(&item.time)) else { continue };
            let when = NSDateComponents::new();
            when.setYear(y);
            when.setMonth(mo);
            when.setDay(d);
            when.setHour(h);
            when.setMinute(mi);
            let trigger =
                UNCalendarNotificationTrigger::triggerWithDateMatchingComponents_repeats(&when, false);
            let content = UNMutableNotificationContent::new();
            content.setTitle(&NSString::from_str(&item.title));
            content.setBody(&NSString::from_str(&item.text));
            let id = NSString::from_str(&format!("kabar.reminder.{i}"));
            let request =
                UNNotificationRequest::requestWithIdentifier_content_trigger(&id, &content, Some(&trigger));
            center.addNotificationRequest_withCompletionHandler(&request, None);
        }
        Ok(())
    }

    /// The "new version" notification's id: a newer one replaces it.
    const UPDATE_ID: &str = "kabar.update";

    /// Shows the "new version" notification now. macOS asks for permission
    /// the first time (only then); while Kabar is the active app it keeps
    /// the notification out of sight, and the in-app banner says it.
    pub fn post_update(n: &crate::update::Notice) -> bool {
        if !bundled() {
            return false;
        }
        request();
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(&n.title));
        content.setBody(&NSString::from_str(&n.body));
        let id = NSString::from_str(UPDATE_ID);
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(&id, &content, None);
        UNUserNotificationCenter::currentNotificationCenter()
            .addNotificationRequest_withCompletionHandler(&request, None);
        true
    }

    /// Takes the "new version" notification out of Notification Centre.
    pub fn clear_update() {
        if !bundled() {
            return;
        }
        let ids = objc2_foundation::NSArray::from_retained_slice(&[NSString::from_str(UPDATE_ID)]);
        UNUserNotificationCenter::currentNotificationCenter()
            .removeDeliveredNotificationsWithIdentifiers(&ids);
    }

    fn ymd(s: &str) -> Option<(isize, isize, isize)> {
        let mut parts = s.split('-').map(|p| p.parse().ok());
        Some((parts.next()??, parts.next()??, parts.next()??))
    }

    fn hm(s: &str) -> Option<(isize, isize)> {
        let (h, m) = s.split_once(':')?;
        Some((h.parse().ok()?, m.parse().ok()?))
    }
}

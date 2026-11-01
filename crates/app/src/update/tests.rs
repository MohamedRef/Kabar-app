//! Existing behavior regressions for the feature facade.

use super::*;

fn offer(version: &str, build: u64) -> Offer {
    Offer {
        version: version.into(),
        build,
        commit: None,
        notes: UpdateNotes::default(),
        asset: UpdateAsset {
            platform: UpdatePlatform::AndroidArm64,
            file: "kabar.apk".into(),
            size: 1,
            sha256: "00".repeat(32),
            version_code: None,
            build: None,
        },
        changes: Vec::new(),
    }
}

fn status(phase: Phase, mode: Mode) -> Status {
    Status {
        off: None,
        phase,
        prefs: Prefs { mode, ..Prefs::default() },
        channel: Channel::Alpha,
        preview: false,
        version: "1.0.0-alpha.1".into(),
        build: 0,
        content_build: 0,
        content_since_ms: None,
        content_waiting: None,
        content_check: None,
        packs_refreshed: 0,
        checking_all: false,
        history: Vec::new(),
    }
}

#[test]
fn prefs_round_trip_and_default_to_automatic_alpha() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(load_prefs(dir.path()), Prefs::default());
    assert_eq!(Prefs::default().mode, Mode::Auto);
    let p = Prefs {
        mode: Mode::Notify,
        channel: Some(Channel::Beta),
        last_check_ms: Some(5),
        ..Prefs::default()
    };
    save_prefs(dir.path(), &p).unwrap();
    assert_eq!(load_prefs(dir.path()), p);
    let release =
        BuildInfo { version: Version::parse("1.0.0").unwrap(), build: 0, preview: false, key: None };
    assert_eq!(release.channel(None), Channel::Alpha);
    assert_eq!(release.channel(Some(Channel::Beta)), Channel::Beta);
    // A release build never follows the preview feed, whatever the file says.
    assert_eq!(release.channel(Some(Channel::Preview)), Channel::Alpha);
    let preview = BuildInfo { preview: true, ..release };
    assert_eq!(preview.channel(Some(Channel::Stable)), Channel::Preview);
}

#[test]
fn the_banner_waits_for_something_to_do_and_can_be_closed() {
    let o = offer("1.0.0-alpha.2", 0);
    assert!(
        status(Phase::Available(o.clone()), Mode::Auto).banner().is_none(),
        "auto mode is still downloading"
    );
    assert!(status(Phase::Available(o.clone()), Mode::Notify).banner().is_some());
    assert!(status(Phase::Ready(o.clone()), Mode::Auto).banner().is_some());
    let mut s = status(Phase::Ready(o.clone()), Mode::Auto);
    s.prefs.dismissed = Some(o.key());
    assert!(s.banner().is_none());
    // A newer offer shows again.
    s.phase = Phase::Ready(offer("1.0.0-alpha.3", 0));
    assert!(s.banner().is_some());
}

#[test]
fn offers_label_previews_by_build() {
    assert_eq!(offer("1.0.0-alpha.2", 0).label(), "1.0.0-alpha.2");
    assert_eq!(offer("0.1.0", 141).label(), "0.1.0 (141)");
    let me = Installed { version: Version::parse("0.1.0").unwrap(), build: 140, channel: Channel::Preview };
    assert!(is_newer(&me, &offer("0.1.0", 141)));
    assert!(!is_newer(&me, &offer("0.1.0", 140)));
}

#[test]
fn staged_offer_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    assert!(load_staged(dir.path()).is_none());
    let o = offer("1.0.0-alpha.2", 0);
    save_staged(dir.path(), &o).unwrap();
    assert_eq!(load_staged(dir.path()), Some(o));
}

/// Serves `dir` over plain HTTP on localhost, one request per connection.
fn serve(dir: PathBuf) -> String {
    serve_logged(dir).0
}

/// A tiny feed server: answers `Range: bytes=N-` with 206, and when
/// `<file>.cut` holds a number, sends only that many bytes of each answer
/// before hanging up (a dropped connection). Logs each request's path
/// and range start.
/// Each request's path and the byte its range starts at.
type RequestLog = std::sync::Arc<Mutex<Vec<(String, u64)>>>;

fn serve_logged(dir: PathBuf) -> (String, RequestLog) {
    use std::io::{BufRead as _, BufReader};
    let log = RequestLog::default();
    let seen = log.clone();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                continue;
            }
            let path = line.split_whitespace().nth(1).unwrap_or("/").trim_start_matches('/').to_owned();
            let mut from = 0u64;
            loop {
                let mut h = String::new();
                if reader.read_line(&mut h).is_err() || h.trim().is_empty() {
                    break;
                }
                let lower = h.to_ascii_lowercase();
                if let Some(r) = lower.strip_prefix("range: bytes=") {
                    from = r.trim().trim_end_matches('-').parse().unwrap_or(0);
                }
            }
            seen.lock().unwrap().push((path.clone(), from));
            let mut stream = reader.into_inner();
            let cut: Option<usize> =
                fs::read_to_string(dir.join(format!("{path}.cut"))).ok().and_then(|c| c.trim().parse().ok());
            let _ = match fs::read(dir.join(&path)) {
                Ok(body) if from as usize >= body.len() && from > 0 => stream.write_all(
                    b"HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                ),
                Ok(body) => {
                    let total = body.len();
                    let rest = &body[from as usize..];
                    let head = if from > 0 {
                        format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {from}-{}/{total}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            total - 1,
                            rest.len()
                        )
                    } else {
                        format!("HTTP/1.1 200 OK\r\nContent-Length: {total}\r\nConnection: close\r\n\r\n")
                    };
                    let sent = &rest[..cut.unwrap_or(rest.len()).min(rest.len())];
                    stream.write_all(head.as_bytes()).and_then(|()| stream.write_all(sent))
                }
                Err(_) => stream
                    .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"),
            };
        }
    });
    (url, log)
}

fn fixture_feed() -> tempfile::TempDir {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/update");
    let feed = tempfile::tempdir().unwrap();
    for name in [MANIFEST_FILE, SIGNATURE_FILE] {
        fs::copy(src.join(name), feed.path().join(name)).unwrap();
    }
    // The signed manifest's Android asset: these bytes (no .apk is tracked).
    fs::write(feed.path().join("kabar-1.0.0-alpha.2-android-arm64.apk"), b"fixture apk bytes").unwrap();
    feed
}

fn updater(feed: &str, version: &str) -> (&'static Updater, tempfile::TempDir) {
    let data = tempfile::tempdir().unwrap();
    (updater_in(data.path(), feed, version), data)
}

fn updater_in(data: &Path, feed: &str, version: &str) -> &'static Updater {
    let key = include_str!("../../../../fixtures/update/test-key.pub");
    let info = BuildInfo {
        version: Version::parse(version).unwrap(),
        build: 0,
        preview: false,
        key: Some(parse_public_key(key).unwrap()),
    };
    let mut u = Updater::new(
        data.join("updates"),
        info,
        Some(UpdatePlatform::AndroidArm64),
        Some(feed.to_owned()),
        None,
    );
    u.retry_after = Duration::from_millis(10);
    Box::leak(Box::new(u))
}

/// Android's job can start the process before any UI (#429): the app's
/// start then takes the job's updater instead of opening a second one.
/// The only test that touches the process-wide updater.
#[test]
fn the_job_and_the_app_share_one_updater() {
    let data = tempfile::tempdir().unwrap();
    let job = job_updater(data.path());
    assert!(get().is_some_and(|u| std::ptr::eq(u, job)), "the job's updater is the app's");
    init(data.path());
    init(data.path());
    assert!(std::ptr::eq(get().unwrap(), job), "init reuses it");
    assert!(std::ptr::eq(job_updater(data.path()), job));
    let local = OnceLock::new();
    assert!(std::ptr::eq(shared(&local, data.path()), shared(&local, data.path())));
}

#[test]
fn whats_new_before_and_after_an_update() {
    let feed = fixture_feed();
    let url = serve(feed.path().to_path_buf());
    let data = tempfile::tempdir().unwrap();

    // A first install says nothing about what's new.
    let u = updater_in(data.path(), &url, "1.0.0-alpha.1");
    assert!(u.status().whats_new().is_empty());
    assert_eq!(u.status().prefs.last_run.as_deref(), Some("1.0.0-alpha.1+0"));

    // Before installing: the offer brings the builds after this one.
    u.check(false);
    let Phase::Available(o) = u.status().phase else { panic!("{:?}", u.status().phase) };
    assert_eq!(o.changes.iter().map(BuildChanges::key).collect::<Vec<_>>(), ["1.0.0-alpha.2+0"]);
    assert_eq!(u.status().history.len(), 2, "the whole feed history is kept");
    let ctx = NoticeContext { lang: UiLanguage::ArEg, ..NoticeContext::default() };
    u.set_notice_context(ctx);
    let n = notice(&u.status(), None, 0).unwrap();
    assert!(n.body.starts_with("• حلقات راديو جديدة\n• القارئ بقى أسرع\n• الصوت"), "{}", n.body);

    // The first launch of the new build opens "What's new" once.
    let u = updater_in(data.path(), &url, "1.0.0-alpha.2");
    let w = u.status().whats_new();
    assert_eq!(w.iter().map(BuildChanges::key).collect::<Vec<_>>(), ["1.0.0-alpha.2+0"]);
    u.close_whats_new();
    assert!(u.status().whats_new().is_empty());
    let u = updater_in(data.path(), &url, "1.0.0-alpha.2");
    assert!(u.status().whats_new().is_empty(), "seen stays seen");
}

#[test]
fn a_content_bundle_comes_without_an_install() {
    use base64::Engine as _;
    use ed25519_dalek::Signer as _;
    let packed = miniz_oxide::deflate::compress_to_vec(b"cast: []\n", 10);
    let bundle = kabar_protocol::bundle::encode(&[("CAST".into(), packed)]);
    let feed = tempfile::tempdir().unwrap();
    let m = UpdateManifest {
        format: kabar_protocol::update::UPDATE_FORMAT,
        channel: Channel::Alpha,
        version: "1.0.0-alpha.1".into(),
        build: 0,
        commit: None,
        published_at: "2026-10-28T08:00:00Z".into(),
        notes: UpdateNotes::default(),
        assets: Vec::new(),
        history: Vec::new(),
        content: Some(kabar_protocol::update::ContentFile {
            file: "kabar-content.bundle".into(),
            size: bundle.len() as u64,
            sha256: kabar_core::packs::sha256_hex(&bundle),
            ui_build: 0,
        }),
    };
    let json = serde_json::to_vec(&m).unwrap();
    let key = ed25519_dalek::SigningKey::from_bytes(&[0x4b; 32]);
    let sig = base64::engine::general_purpose::STANDARD.encode(key.sign(&json).to_bytes());
    fs::write(feed.path().join(MANIFEST_FILE), &json).unwrap();
    fs::write(feed.path().join(SIGNATURE_FILE), sig).unwrap();
    fs::write(feed.path().join("kabar-content.bundle"), &bundle).unwrap();
    let url = serve(feed.path().to_path_buf());

    let (u, data) = updater(&url, "1.0.0-alpha.1");
    // "Check for updates" by hand: the feed, the content, then the packs.
    u.check_everything();
    assert_eq!(u.status().phase, Phase::UpToDate, "the same build: nothing to install");
    assert_eq!(u.status().content_waiting, Some(m.build));
    assert_eq!(u.status().content_check, Some(content::Refresh::Taken(m.build)));
    assert!(!u.status().checking_all, "the check by hand is over");
    assert_eq!(u.status().packs_refreshed, 0, "no audio packs on this device");
    let (entries, look) = content::load(data.path(), &u.info).unwrap();
    assert_eq!((entries.len(), look), (1, true), "the look fits the build it was made with");
    // Taken once; a newer app drops it.
    let again = updater_in(data.path(), &url, "1.0.0-alpha.1");
    again.check(false);
    assert_eq!(again.status().content_waiting, None);
    assert_eq!(
        again.status().content_check,
        Some(content::Refresh::Current),
        "nothing newer than what's in use"
    );
    let newer = BuildInfo { version: Version::parse("1.0.0-alpha.2").unwrap(), ..u.info.clone() };
    assert!(content::load(data.path(), &newer).is_none());
    assert!(content::load(data.path(), &u.info).is_none(), "and it is gone");
}

#[test]
fn checks_downloads_and_verifies_from_a_signed_feed() {
    let feed = fixture_feed();
    let url = serve(feed.path().to_path_buf());

    let (u, _data) = updater(&url, "1.0.0-alpha.1");
    u.check(true);
    let Phase::Ready(offer) = u.status().phase else { panic!("{:?}", u.status().phase) };
    assert_eq!(offer.version, "1.0.0-alpha.2");
    assert_eq!(offer.asset.version_code, Some(10_000_102));
    assert!(u.verified_staged(&offer).is_ok());
    assert!(u.status().prefs.last_check_ms.is_some());
    // A fresh start finds the staged download again.
    let again = Updater::new(u.dir.clone(), u.info.clone(), u.platform, u.feed_override.clone(), None);
    assert_eq!(again.status().phase, Phase::Ready(offer.clone()));
    // Changed on disk after the download: refused at install time.
    fs::write(u.dir.join(&offer.asset.file), b"fixture apk bytez").unwrap();
    assert!(matches!(u.verified_staged(&offer), Err(Failure::Verify(_))));

    // Already on that version: nothing to do, and old downloads go.
    let (u, _data) = updater(&url, "1.0.0-alpha.2");
    u.check(true);
    assert_eq!(u.status().phase, Phase::UpToDate);
}

#[test]
fn a_notice_goes_out_once_per_offer_and_step() {
    let noon = Some(LocalTime::hm(12, 0));
    let mut o = offer("1.0.0-alpha.2", 0);
    o.notes = UpdateNotes { en: "Javanese Sinetron, episode 12".into(), ..UpdateNotes::default() };
    let mut s = status(Phase::Available(o.clone()), Mode::Notify);
    s.prefs.notice = Some(NoticeContext { lang: UiLanguage::En, ..NoticeContext::default() });
    let n = notice(&s, noon, 0).expect("a new version is announced");
    assert_eq!(n.title, "A new Kabar is out · 1.0.0-alpha.2");
    assert!(n.body.starts_with("What's new: Javanese Sinetron, episode 12\n"), "{}", n.body);
    assert_eq!((n.action, n.button.as_str()), ("download", "Download"));
    // Sent once: the same offer at the same step stays quiet.
    s.prefs.notified = Some(n.key.clone());
    assert!(notice(&s, noon, 0).is_none());
    // Downloaded since: "ready to install" is a new step.
    s.phase = Phase::Ready(o.clone());
    let ready = notice(&s, noon, 0).unwrap();
    assert_eq!(ready.action, "install");
    assert_ne!(ready.key, n.key);
    // A closed banner, the switch, or updates off: nothing.
    let mut closed = s.clone();
    closed.prefs.dismissed = Some(o.key());
    assert!(notice(&closed, noon, 0).is_none());
    let mut quiet = s.clone();
    quiet.prefs.notify = false;
    assert!(notice(&quiet, noon, 0).is_none());
    let mut off = s.clone();
    off.prefs.mode = Mode::Off;
    assert!(notice(&off, noon, 0).is_none());
    // Nothing to say while checking or up to date.
    assert!(notice(&status(Phase::UpToDate, Mode::Auto), noon, 0).is_none());
}

#[test]
fn notices_keep_quiet_hours_and_speak_the_learners_language() {
    let mut s = status(Phase::Ready(offer("0.1.0", 141)), Mode::Auto);
    let quiet = QuietHours { start: LocalTime::hm(22, 30), end: LocalTime::hm(8, 0) };
    s.prefs.notice = Some(NoticeContext { lang: UiLanguage::ArEg, quiet: Some(quiet), utc_offset_min: 120 });
    assert!(notice(&s, Some(LocalTime::hm(23, 15)), 0).is_none(), "inside quiet hours");
    assert!(notice(&s, Some(LocalTime::hm(7, 59)), 0).is_none());
    let n = notice(&s, Some(LocalTime::hm(8, 0)), 0).expect("quiet hours are over");
    assert!(n.title.starts_with("نسخة Kabar الجديدة جاهزة"), "{}", n.title);
    assert!(n.title.ends_with("0.1.0 (141)"));
    // No notes: the tap hint alone.
    assert_eq!(n.body, text::ut(UiLanguage::ArEg, text::U::NoticeTapReady));
    // Without a local time, the stored offset decides: 21:00 UTC is 23:00 in Cairo.
    assert!(notice(&s, None, 21 * 3600 * 1000).is_none());
    assert!(notice(&s, None, 12 * 3600 * 1000).is_some());
    // The prefs file keeps all of it, and old files default the switch on.
    let dir = tempfile::tempdir().unwrap();
    save_prefs(dir.path(), &s.prefs).unwrap();
    assert_eq!(load_prefs(dir.path()), s.prefs);
    fs::write(dir.path().join("prefs.json"), r#"{"mode":"notify"}"#).unwrap();
    assert!(load_prefs(dir.path()).notify);
}

#[test]
fn the_background_check_downloads_on_wifi_and_announces_once() {
    let feed = fixture_feed();
    let url = serve(feed.path().to_path_buf());
    let noon = LocalTime::hm(12, 0);

    // Automatic mode on Wi-Fi: downloaded, then "ready to install".
    let (u, _data) = updater(&url, "1.0.0-alpha.1");
    let n = u.background(false, true, noon).expect("announced");
    assert_eq!(n.action, "install");
    assert!(matches!(u.status().phase, Phase::Ready(_)));
    // Not shown (Android said no): offered again at the next run.
    assert_eq!(u.status().prefs.notified, None, "marked only once shown");
    assert_eq!(u.background(false, true, noon).as_ref(), Some(&n));
    u.shown(&n.key);
    assert_eq!(u.status().prefs.notified.as_deref(), Some(n.key.as_str()));
    assert!(u.background(false, true, noon).is_none(), "once shown, never again");
    assert!(serde_json::to_string(&n).unwrap().contains(&format!("\"key\":\"{}\"", n.key)));

    // On mobile data: announced, not downloaded.
    let (u, _data) = updater(&url, "1.0.0-alpha.1");
    let n = u.background(true, true, noon).unwrap();
    assert_eq!(n.action, "download");
    assert!(matches!(u.status().phase, Phase::Available(_)));

    // Kabar in front (or notifications off): checked, kept for later.
    let (u, _data) = updater(&url, "1.0.0-alpha.1");
    assert!(u.background(true, false, noon).is_none());
    assert!(u.status().prefs.notified.is_none());
    assert!(u.background(true, true, noon).is_some(), "the next run sends it");

    // Up to date: nothing.
    let (u, _data) = updater(&url, "1.0.0-alpha.2");
    assert!(u.background(false, true, noon).is_none());
}

#[test]
fn a_dropped_download_picks_up_where_it_stopped() {
    let apk = "kabar-1.0.0-alpha.2-android-arm64.apk";
    let feed = fixture_feed();
    let cut = feed.path().join(format!("{apk}.cut"));
    let (url, log) = serve_logged(feed.path().to_path_buf());
    let ranges = || -> Vec<u64> {
        let mut log = log.lock().unwrap();
        let r = log.iter().filter(|(p, _)| p == apk).map(|(_, from)| *from).collect();
        log.clear();
        r
    };

    // Every answer stops after 6 bytes: the 17-byte build comes in three
    // pieces, each try asking for the rest, and the whole is verified.
    fs::write(&cut, "6").unwrap();
    let (u, data) = updater(&url, "1.0.0-alpha.1");
    u.check(false);
    u.download();
    assert!(matches!(u.status().phase, Phase::Ready(_)), "{:?}", u.status().phase);
    assert_eq!(ranges(), [0, 6, 12]);
    let dir = data.path().join("updates");
    assert_eq!(fs::read(dir.join(apk)).unwrap(), b"fixture apk bytes");
    assert!(!dir.join(format!("{apk}.part")).exists());
    assert!(!dir.join(format!("{apk}.part.json")).exists());

    // Out of quick retries (2 bytes a try): the part stays, and the next
    // try, a tap or the next check later, continues it.
    fs::write(&cut, "2").unwrap();
    let (u, data) = updater(&url, "1.0.0-alpha.1");
    u.check(false);
    u.download();
    assert!(matches!(u.status().phase, Phase::Failed { why: Failure::Network, .. }));
    assert_eq!(ranges(), [0, 2, 4, 6]);
    let part = data.path().join("updates").join(format!("{apk}.part"));
    assert_eq!(fs::read(&part).unwrap(), b"fixture ");
    fs::remove_file(&cut).unwrap();
    u.download();
    assert!(matches!(u.status().phase, Phase::Ready(_)), "{:?}", u.status().phase);
    assert_eq!(ranges(), [8]);

    // A damaged part of this build: the resumed file fails the hash, so
    // it's fetched whole once more.
    let (u, data) = updater(&url, "1.0.0-alpha.1");
    u.check(false);
    let Phase::Available(o) = u.status().phase else { panic!() };
    let dir = data.path().join("updates");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{apk}.part")), b"FIXTURE ").unwrap();
    let meta = PartMeta { size: o.asset.size, sha256: o.asset.sha256.clone() };
    fs::write(dir.join(format!("{apk}.part.json")), serde_json::to_vec(&meta).unwrap()).unwrap();
    u.download();
    assert!(matches!(u.status().phase, Phase::Ready(_)), "{:?}", u.status().phase);
    assert_eq!(ranges(), [8, 0]);

    // A part left by another build is never continued: it starts over.
    let (u, data) = updater(&url, "1.0.0-alpha.1");
    let dir = data.path().join("updates");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{apk}.part")), b"old buil").unwrap();
    fs::write(dir.join(format!("{apk}.part.json")), r#"{"size":17,"sha256":"00"}"#).unwrap();
    u.check(false);
    u.download();
    assert!(matches!(u.status().phase, Phase::Ready(_)), "{:?}", u.status().phase);
    assert_eq!(ranges(), [0]);
}

#[test]
fn a_swapped_build_or_manifest_is_never_offered() {
    // The build on the server isn't the one the manifest signed.
    let feed = fixture_feed();
    fs::write(feed.path().join("kabar-1.0.0-alpha.2-android-arm64.apk"), b"evil apk bytes!!!").unwrap();
    let url = serve(feed.path().to_path_buf());
    let (u, data) = updater(&url, "1.0.0-alpha.1");
    u.check(true);
    assert!(
        matches!(u.status().phase, Phase::Failed { why: Failure::Verify(_), .. }),
        "{:?}",
        u.status().phase
    );
    let mut left: Vec<_> =
        fs::read_dir(data.path().join("updates")).unwrap().flatten().map(|e| e.file_name()).collect();
    left.sort();
    // The signed manifest's history is kept; the build it didn't sign isn't.
    assert_eq!(left, ["history.json", "prefs.json"], "nothing unverified stays on disk");

    // The manifest changed after signing.
    let feed = fixture_feed();
    let json = fs::read_to_string(feed.path().join(MANIFEST_FILE)).unwrap();
    fs::write(feed.path().join(MANIFEST_FILE), json.replace("alpha.2\"", "alpha.9\"")).unwrap();
    let url = serve(feed.path().to_path_buf());
    let (u, _data) = updater(&url, "1.0.0-alpha.1");
    u.check(true);
    assert!(matches!(u.status().phase, Phase::Failed { why: Failure::Verify(_), .. }));

    // No connection.
    let (u, _data) = updater("http://127.0.0.1:9", "1.0.0-alpha.1");
    u.check(false);
    assert!(matches!(u.status().phase, Phase::Failed { why: Failure::Network, .. }));
}

#[test]
fn content_pointer_survives_an_interrupted_bundle_download() {
    let data = tempfile::tempdir().unwrap();
    let dir = data.path().join("content");
    fs::create_dir_all(&dir).unwrap();
    let bytes = kabar_protocol::bundle::encode(&[(
        "CAST".to_owned(),
        miniz_oxide::deflate::compress_to_vec(b"cast: []\n", 10),
    )]);
    let sha = kabar_core::packs::sha256_hex(&bytes);
    fs::write(dir.join("content.bundle"), &bytes).unwrap();
    let old = content::BundleInfo {
        version: "1.0.0-alpha.1".into(),
        build: 0,
        sha256: sha.clone(),
        ui_build: 0,
        fetched_ms: None,
    };
    fs::write(dir.join("content.json"), serde_json::to_vec(&old).unwrap()).unwrap();
    let info =
        BuildInfo { version: Version::parse("1.0.0-alpha.1").unwrap(), build: 0, preview: false, key: None };
    assert!(content::load(data.path(), &info).is_some(), "legacy file works");
    // A download completed, but the process ended before its metadata
    // pointer was committed. The selected bundle must stay intact.
    fs::write(dir.join(format!("content.{}.bundle", "f".repeat(64))), b"partial").unwrap();
    fs::write(dir.join("content.json.part"), b"incomplete json").unwrap();
    assert!(content::load(data.path(), &info).is_some(), "uncommitted files are ignored");
    assert_eq!(content::in_use(data.path(), &info), Some(old));
}

#[test]
fn corrupt_content_recovers_the_previous_verified_bundle() {
    let data = tempfile::tempdir().unwrap();
    let dir = data.path().join("content");
    fs::create_dir_all(&dir).unwrap();
    let bytes = kabar_protocol::bundle::encode(&[(
        "CAST".to_owned(),
        miniz_oxide::deflate::compress_to_vec(b"cast: []\n", 10),
    )]);
    let sha = kabar_core::packs::sha256_hex(&bytes);
    fs::write(dir.join("content.bundle"), &bytes).unwrap();
    let previous = content::BundleInfo {
        version: "1.0.0-alpha.1".into(),
        build: 0,
        sha256: sha.clone(),
        ui_build: 0,
        fetched_ms: None,
    };
    fs::write(dir.join("content.previous.json"), serde_json::to_vec(&previous).unwrap()).unwrap();
    let current = content::BundleInfo { sha256: "f".repeat(64), ..previous.clone() };
    fs::write(dir.join("content.json"), serde_json::to_vec(&current).unwrap()).unwrap();
    fs::write(dir.join(format!("content.{}.bundle", current.sha256)), b"damaged").unwrap();
    let info =
        BuildInfo { version: Version::parse("1.0.0-alpha.1").unwrap(), build: 0, preview: false, key: None };
    assert!(content::load(data.path(), &info).is_some(), "verified previous content is recovered");
    assert_eq!(content::in_use(data.path(), &info), Some(previous.clone()));
    assert!(content::load(data.path(), &info).is_some(), "restored selection survives restart");

    // Damaging both selections must never activate unverified content.
    fs::write(dir.join("content.bundle"), b"broken too").unwrap();
    fs::write(dir.join("content.json"), serde_json::to_vec(&current).unwrap()).unwrap();
    assert!(content::load(data.path(), &info).is_none(), "fall back to embedded content");
}

#[test]
fn content_refresh_keeps_a_verified_fallback_before_switching() {
    let data = tempfile::tempdir().unwrap();
    let dir = data.path().join("content");
    fs::create_dir_all(&dir).unwrap();
    let old_bytes = kabar_protocol::bundle::encode(&[(
        "CAST".to_owned(),
        miniz_oxide::deflate::compress_to_vec(b"cast: []\n", 10),
    )]);
    let previous = content::BundleInfo {
        version: "1.0.0-alpha.1".into(),
        build: 0,
        sha256: kabar_core::packs::sha256_hex(&old_bytes),
        ui_build: 0,
        fetched_ms: None,
    };
    fs::write(dir.join("content.bundle"), &old_bytes).unwrap();
    fs::write(dir.join("content.json"), serde_json::to_vec(&previous).unwrap()).unwrap();

    let next_bytes = kabar_protocol::bundle::encode(&[(
        "CAST".to_owned(),
        miniz_oxide::deflate::compress_to_vec(b"cast: [new]\n", 10),
    )]);
    let next_sha = kabar_core::packs::sha256_hex(&next_bytes);
    let feed = tempfile::tempdir().unwrap();
    fs::write(feed.path().join("next.bundle"), &next_bytes).unwrap();
    let url = serve(feed.path().to_path_buf());
    let info =
        BuildInfo { version: Version::parse("1.0.0-alpha.1").unwrap(), build: 0, preview: false, key: None };
    let manifest = UpdateManifest {
        format: kabar_protocol::update::UPDATE_FORMAT,
        channel: Channel::Alpha,
        version: "1.0.0-alpha.1".into(),
        build: 1,
        commit: None,
        published_at: "2026-11-01T00:00:00Z".into(),
        notes: UpdateNotes::default(),
        assets: Vec::new(),
        history: Vec::new(),
        content: Some(kabar_protocol::update::ContentFile {
            file: "next.bundle".into(),
            size: next_bytes.len() as u64,
            sha256: next_sha.clone(),
            ui_build: 0,
        }),
    };
    assert_eq!(
        content::refresh(data.path(), &info.installed(Some(Channel::Alpha)), &info, &manifest, &url),
        content::Refresh::Taken(1),
    );
    let saved: content::BundleInfo =
        serde_json::from_slice(&fs::read(dir.join("content.previous.json")).unwrap()).unwrap();
    assert_eq!(saved, previous);
    assert!(content::load(data.path(), &info).is_some(), "new selection is readable");
    fs::remove_file(dir.join(format!("content.{next_sha}.bundle"))).unwrap();
    assert!(content::load(data.path(), &info).is_some(), "failed new bundle recovers old one");
    assert_eq!(content::in_use(data.path(), &info), Some(previous));
}

#[test]
fn content_cleanup_keeps_both_selections_and_legacy_files() {
    let data = tempfile::tempdir().unwrap();
    let dir = data.path().join("content");
    fs::create_dir_all(&dir).unwrap();
    let info =
        BuildInfo { version: Version::parse("1.0.0-alpha.1").unwrap(), build: 0, preview: false, key: None };
    let make = |text: &[u8]| {
        kabar_protocol::bundle::encode(&[(
            "CAST".to_owned(),
            miniz_oxide::deflate::compress_to_vec(text, 10),
        )])
    };
    let current_bytes = make(b"cast: []\n");
    let previous_bytes = make(b"cast: [previous]\n");
    let orphan_bytes = make(b"cast: [orphan]\n");
    let current_sha = kabar_core::packs::sha256_hex(&current_bytes);
    let previous_sha = kabar_core::packs::sha256_hex(&previous_bytes);
    let orphan_sha = kabar_core::packs::sha256_hex(&orphan_bytes);
    let bundle = |sha: &str| dir.join(format!("content.{sha}.bundle"));
    for (sha, bytes) in
        [(&current_sha, &current_bytes), (&previous_sha, &previous_bytes), (&orphan_sha, &orphan_bytes)]
    {
        fs::write(bundle(sha), bytes).unwrap();
    }
    let select = |sha: &str| content::BundleInfo {
        version: "1.0.0-alpha.1".into(),
        build: 0,
        sha256: sha.into(),
        ui_build: 0,
        fetched_ms: None,
    };
    fs::write(dir.join("content.json"), serde_json::to_vec(&select(&current_sha)).unwrap()).unwrap();
    fs::write(dir.join("content.previous.json"), serde_json::to_vec(&select(&previous_sha)).unwrap())
        .unwrap();
    fs::write(dir.join("content.bundle"), b"legacy file").unwrap();
    for name in ["content.bundle.part", "content.json.part", "content.previous.json.part"] {
        fs::write(dir.join(name), b"abandoned").unwrap();
    }
    fs::write(dir.join("unrelated.json"), b"leave alone").unwrap();
    assert!(content::load(data.path(), &info).is_some());
    assert!(bundle(&current_sha).exists(), "active bundle is retained");
    assert!(bundle(&previous_sha).exists(), "previous verified bundle is retained");
    assert!(!bundle(&orphan_sha).exists(), "unreferenced bundle is removed");
    assert!(dir.join("content.bundle").exists(), "legacy compatibility is retained");
    assert!(dir.join("unrelated.json").exists());
    for name in ["content.bundle.part", "content.json.part", "content.previous.json.part"] {
        assert!(!dir.join(name).exists(), "abandoned temporary write is removed");
    }

    // Corrupt metadata must fail closed: no speculative deletion.
    fs::write(dir.join("content.previous.json"), b"incomplete").unwrap();
    fs::write(bundle(&orphan_sha), &orphan_bytes).unwrap();
    assert!(content::load(data.path(), &info).is_some());
    assert!(bundle(&orphan_sha).exists(), "unreadable backup disables cleanup");
}

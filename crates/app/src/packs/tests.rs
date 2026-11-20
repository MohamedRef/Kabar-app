use kabar_core::packs::{PackInput, build_manifest, refresh_totals, summary};
use kabar_protocol::lesson::LessonContent;

use super::*;

fn lesson() -> LessonContent {
    let yaml = crate::lesson::lesson_content("bipa1.u01.l01").expect("bipa1.u01.l01 ships");
    kabar_core::lesson::parse_lesson(&yaml).unwrap()
}

fn tar_of(m: &PackManifest, clips: &[Vec<u8>]) -> Vec<u8> {
    let mut b = tar::Builder::new(Vec::new());
    let mut add = |name: &str, data: &[u8]| {
        let mut h = tar::Header::new_ustar();
        h.set_size(data.len() as u64);
        h.set_mode(0o644);
        b.append_data(&mut h, name, data).unwrap();
    };
    add("manifest.json", &serde_json::to_vec(m).unwrap());
    for (a, bytes) in m.audio.iter().zip(clips) {
        add(&format!("audio/{}.mp3", a.key), bytes);
    }
    b.into_inner().unwrap()
}

/// A manifest with fake clips, its index row and its tar (the layout
/// `kabar-tools packs bundle` writes).
pub(super) fn pack() -> (PackManifest, Vec<Vec<u8>>, PackSummary, Vec<u8>) {
    pack_v(1)
}

/// [`pack`] at `version`.
pub(super) fn pack_v(version: u32) -> (PackManifest, Vec<Vec<u8>>, PackSummary, Vec<u8>) {
    let l = lesson();
    let mut m = build_manifest(
        "id",
        "bipa1.u01",
        &[PackInput { id: "bipa1.u01.l01", yaml_sha256: "x".into(), lesson: &l }],
        None,
    );
    m.version = version;
    let clips: Vec<Vec<u8>> =
        (0..m.audio.len()).map(|i| format!("clip {version}:{i}").into_bytes()).collect();
    for (a, bytes) in m.audio.iter_mut().zip(&clips) {
        a.sha256 = Some(sha256_hex(bytes));
        a.size = Some(bytes.len() as u64);
    }
    refresh_totals(&mut m);
    let tar = tar_of(&m, &clips);
    let mut row = summary(&m, None);
    row.file_sha256 = Some(sha256_hex(&tar));
    row.file_size = Some(tar.len() as u64);
    (m, clips, row, tar)
}

#[test]
fn install_find_and_remove() {
    let dir = tempfile::tempdir().unwrap();
    let store = PackStore::new(dir.path());
    let (m, _, row, tar) = pack();
    assert!(store.installed().is_empty());

    store.install(&row, &tar).unwrap();
    assert_eq!(store.installed_versions().get("id/bipa1.u01"), Some(&1));
    let first = &m.audio[0];
    let found = store.clip(first.voice, &format!("  {} ", first.text)).expect("clip");
    assert_eq!(fs::read(found).unwrap(), b"clip 1:0");
    assert!(store.storage_bytes() > 0);
    // The lesson's narrator lines use the default voice.
    let l = lesson();
    let narrated = &l.slides.iter().flat_map(|s| &s.examples).next().unwrap().text;
    assert!(store.clip(default_voice("id"), narrated).is_some());

    store.remove("bipa1.u01").unwrap();
    assert!(store.installed().is_empty());
    assert_eq!(store.storage_bytes(), 0);
}

#[test]
fn pack_store_never_touches_or_counts_the_cloud_voice_cache() {
    let dir = tempfile::tempdir().unwrap();
    let cloud_audio = dir.path().join("audio");
    fs::create_dir_all(cloud_audio.join("episodes")).unwrap();
    let cloud_clip = cloud_audio.join(format!("{}.mp3", "a".repeat(64)));
    let radio_clip = cloud_audio.join("episodes/radio.bipa1.u01.mp3");
    fs::write(&cloud_clip, b"neural voice clip").unwrap();
    fs::write(&radio_clip, b"radio track").unwrap();

    let store = PackStore::open(dir.path());
    assert_eq!(store.storage_bytes(), 0, "cloud TTS clips live outside packs/audio");

    let (_, _, row, tar) = pack();
    store.install(&row, &tar).unwrap();
    store.remove("bipa1.u01").unwrap();
    assert_eq!(store.storage_bytes(), 0);
    assert_eq!(fs::read(&cloud_clip).unwrap(), b"neural voice clip", "cloud TTS survives pack removal");
    assert_eq!(fs::read(&radio_clip).unwrap(), b"radio track");
}

#[test]
fn opening_migrates_legacy_pack_clips_and_sweeps_orphans_and_parts() {
    let dir = tempfile::tempdir().unwrap();
    let (m, clips, _, _) = pack();
    // Simulate an older build: manifest in `packs/`, pack clips mixed into `audio/` alongside cloud TTS.
    fs::create_dir_all(dir.path().join("packs")).unwrap();
    fs::create_dir_all(dir.path().join("audio")).unwrap();
    fs::write(dir.path().join("packs/bipa1.u01.json"), serde_json::to_vec(&m).unwrap()).unwrap();
    for (a, bytes) in m.audio.iter().zip(&clips) {
        fs::write(dir.path().join(format!("audio/{}.mp3", a.key)), bytes).unwrap();
    }
    let cloud_clip = dir.path().join(format!("audio/{}.mp3", "b".repeat(64)));
    fs::write(&cloud_clip, b"cloud tts").unwrap();
    // Stray orphan clip and interrupted `.part` files inside `packs/`.
    fs::create_dir_all(dir.path().join("packs/audio")).unwrap();
    let stray = dir.path().join(format!("packs/audio/{}.mp3", "c".repeat(64)));
    let part = dir.path().join("packs/audio/.tmp.1.0.part");
    fs::write(&stray, b"stray").unwrap();
    fs::write(&part, b"half written").unwrap();

    let store = PackStore::open(dir.path());
    assert!(store.clip(m.audio[0].voice, &m.audio[0].text).is_some(), "migrated into packs/audio");
    assert!(!dir.path().join(format!("audio/{}.mp3", m.audio[0].key)).exists(), "moved out of audio/");
    assert_eq!(fs::read(&cloud_clip).unwrap(), b"cloud tts", "unlisted cloud clip left alone");
    assert!(!stray.exists() && !part.exists(), "orphan and .part swept on open");
}

#[test]
fn damaged_downloads_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let store = PackStore::new(dir.path());
    let (m, mut clips, row, mut tar) = pack();
    // Not the tar the index announced.
    let at = tar.len() / 2;
    tar[at] ^= 1;
    assert!(store.install(&row, &tar).unwrap_err().contains("hash"));
    // The announced tar, but a clip that doesn't match its manifest hash.
    clips[0] = b"tampered".to_vec();
    let bad = tar_of(&m, &clips);
    let row = PackSummary { file_sha256: Some(sha256_hex(&bad)), ..row };
    assert!(store.install(&row, &bad).unwrap_err().contains("damaged"));
    // A manifest that isn't the one the index lists.
    let (_, clips, row, _) = pack();
    let mut other = m.clone();
    other.version = 7;
    let t = tar_of(&other, &clips);
    let row = PackSummary { file_sha256: Some(sha256_hex(&t)), ..row };
    assert!(store.install(&row, &t).is_err());
    // Unsafe unit ids are refused in both install and remove.
    let bad_row = PackSummary { unit: "BIPA1/u01".into(), ..row };
    assert_eq!(store.install(&bad_row, &t).unwrap_err(), "bad unit id");
    assert_eq!(store.remove("BIPA1/u01").unwrap_err(), "bad unit id");
    assert!(store.installed().is_empty());
    assert_eq!(store.storage_bytes(), 0);
}

#[test]
fn the_queue_runs_in_order_and_keeps_failures_for_a_retry() {
    let jobs = Mutex::new(Vec::new());
    for id in ["a", "b", "c", "a"] {
        queue(&jobs, id);
    }
    let mut order = Vec::new();
    run_queue(&jobs, |id| {
        order.push(id.to_owned());
        if id == "b" { Err("network".into()) } else { Ok(()) }
    });
    assert_eq!(order, ["a", "b", "c"], "one at a time, asked twice runs once");
    assert_eq!(*jobs.lock().unwrap(), [("b".to_owned(), Job::Failed)]);
    queue(&jobs, "b");
    assert_eq!(*jobs.lock().unwrap(), [("b".to_owned(), Job::Queued)], "retry");
    run_queue(&jobs, |_| Ok(()));
    assert!(jobs.lock().unwrap().is_empty());

    // An item already in flight on another thread isn't recorded as Failed.
    queue(&jobs, "d");
    run_queue(&jobs, |_| Err(ALREADY_DOWNLOADING.into()));
    assert!(jobs.lock().unwrap().is_empty());
}

#[test]
fn auto_download_defaults_on() {
    let dir = tempfile::tempdir().unwrap();
    let store = PackStore::new(dir.path());
    assert!(store.auto());
    store.set_auto(false).unwrap();
    assert!(!store.auto());
    // Prefs saved before the switch existed (or damaged) mean on.
    fs::write(dir.path().join("packs/prefs.json"), "{}").unwrap();
    assert!(store.auto());
    fs::write(dir.path().join("packs/prefs.json"), "not json").unwrap();
    assert!(store.auto());
}

#[test]
fn without_a_pack_store_nothing_is_fetched() {
    // The app opens its store at start; before that (and in these
    // tests) prefetching and updating quietly do nothing.
    assert!(store().is_none());
    assert_eq!(update_installed(true), 0);
    prefetch_for("bipa1.u01.l01");
    assert_eq!(clip_url("Selamat pagi", "id", None), None);
    assert!(jobs().is_empty() || jobs().values().all(|j| *j != Job::Queued));
}

#[test]
fn embedded_index_lists_every_unit() {
    let index = embedded_index();
    assert!(index.packs.iter().any(|p| p.id == "id/bipa1.u01"));
    assert!(index.packs.iter().any(|p| p.id == "jv/j1.u01"));
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(PackStore::new(dir.path()).index(), index);
}

#[test]
fn remote_pack_urls_must_use_https() {
    assert!(is_allowed_packs_url(DEFAULT_PACKS_URL));
    let plain = DEFAULT_PACKS_URL.replacen("https", "http", 1);
    assert!(!is_allowed_packs_url(&plain));
    assert!(!is_allowed_packs_url(""));
}

// ------------------------------------------------------- over the network

/// Serves `files` over plain HTTP on localhost (404 for anything else),
/// and logs each path asked for.
pub(super) fn serve(files: BTreeMap<String, Vec<u8>>) -> (String, std::sync::Arc<Mutex<Vec<String>>>) {
    use std::io::{BufRead as _, BufReader, Write as _};
    let log = std::sync::Arc::new(Mutex::new(Vec::new()));
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
            loop {
                let mut h = String::new();
                if reader.read_line(&mut h).is_err() || h.trim().is_empty() {
                    break;
                }
            }
            seen.lock().unwrap().push(path.clone());
            let mut stream = reader.into_inner();
            let _ = match files.get(&path) {
                Some(body) => stream
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .and_then(|()| stream.write_all(body)),
                None => stream
                    .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"),
            };
        }
    });
    (url, log)
}

/// Downloads share `IN_FLIGHT` and the job queue: tests that download
/// take turns, so one never sees another's "already downloading".
pub(super) fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
    static TURN: Mutex<()> = Mutex::new(());
    TURN.lock().unwrap_or_else(PoisonError::into_inner)
}

fn index_of(rows: Vec<PackSummary>) -> PackIndex {
    PackIndex { format: PACK_FORMAT, packs: rows }
}

/// A release holding `rows` as its index and each `(file, tar)`.
pub(super) fn release(
    rows: Vec<PackSummary>,
    tars: &[(&str, &[u8])],
) -> (String, std::sync::Arc<Mutex<Vec<String>>>) {
    let mut files = BTreeMap::new();
    files.insert("index.json".to_owned(), serde_json::to_vec(&index_of(rows)).unwrap());
    for (name, tar) in tars {
        files.insert((*name).to_owned(), tar.to_vec());
    }
    serve(files)
}

fn store_in(dir: &tempfile::TempDir) -> PackStore {
    PackStore::new(dir.path())
}

#[test]
fn a_fetched_index_is_remembered_and_wins_over_the_built_in_one() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_in(&dir);
    let (_, _, row, tar) = pack();
    let (url, log) = release(vec![row.clone()], &[(&row.file, &tar)]);
    let index = refresh_index_from(&store, &url).unwrap();
    assert_eq!(index.packs, vec![row.clone()]);
    assert_eq!(store.index(), index, "the release's index, not the built-in one");
    assert_eq!(*log.lock().unwrap(), ["index.json"]);
    // A fetched index with nothing in it never hides the built-in units.
    let (empty, _) = release(Vec::new(), &[]);
    refresh_index_from(&store, &empty).unwrap();
    assert_eq!(store.index(), embedded_index());
}

#[test]
fn index_failures_say_what_went_wrong() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_in(&dir);
    // Nothing published there.
    let (url, _) = serve(BTreeMap::new());
    assert!(refresh_index_from(&store, &url).unwrap_err().ends_with("HTTP 404"));
    // Not an index.
    let mut files = BTreeMap::new();
    files.insert("index.json".to_owned(), b"<html>".to_vec());
    let (url, _) = serve(files);
    assert!(refresh_index_from(&store, &url).unwrap_err().starts_with("index:"));
    // Made for a newer app: refused, and not remembered.
    let newer = PackIndex { format: PACK_FORMAT + 1, packs: vec![pack().2] };
    let mut files = BTreeMap::new();
    files.insert("index.json".to_owned(), serde_json::to_vec(&newer).unwrap());
    let (url, _) = serve(files);
    assert_eq!(refresh_index_from(&store, &url).unwrap_err(), TOO_NEW);
    assert_eq!(store.index(), embedded_index());
    // Nobody listening at all.
    let closed = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", l.local_addr().unwrap())
    };
    assert!(refresh_index_from(&store, &closed).unwrap_err().starts_with("network:"));
}

#[test]
fn a_download_is_checked_then_installed_and_plays() {
    let _turn = one_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let store = store_in(&dir);
    let (m, clips, row, tar) = pack();
    let (url, log) = release(vec![row.clone()], &[(&row.file, &tar)]);
    refresh_index_from(&store, &url).unwrap();
    let installed = download_from(&store, Some(&url), &row.id).unwrap();
    assert_eq!(installed, m);
    assert_eq!(store.installed_versions().get(&row.id), Some(&1));
    assert_eq!(log.lock().unwrap().last().map(String::as_str), Some(row.file.as_str()));
    // The Lesson Player gets the clip as a data URL for <audio>.
    let first = &m.audio[0];
    let url = clip_url_in(&store, &first.text, "id", Some(first.voice)).unwrap();
    let b64 = base64::engine::general_purpose::STANDARD.encode(&clips[0]);
    assert_eq!(url, format!("data:{AUDIO_MIME};base64,{b64}"));
    assert_eq!(clip_url_in(&store, "a line no pack has", "id", None), None);
    // Nothing is left running.
    assert!(!jobs().contains_key(&row.id));
}

#[test]
fn stale_saved_index_refreshes_once_when_release_tar_was_reuploaded() {
    let _turn = one_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let store = store_in(&dir);
    let (_, _, row1, _) = pack_v(1);
    store.save_index(&index_of(vec![row1.clone()])).unwrap();

    // Release now serves v2 at the same `<unit>.tar` URL.
    let (m2, _, row2, tar2) = pack_v(2);
    let (url, log) = release(vec![row2.clone()], &[(&row2.file, &tar2)]);
    let installed = download_from(&store, Some(&url), &row1.id).unwrap();
    assert_eq!(installed, m2);
    assert_eq!(store.installed_versions().get(&row2.id), Some(&2));
    assert_eq!(
        *log.lock().unwrap(),
        [row2.file, "index.json".to_owned()],
        "reuses the already-downloaded v2 tar once the refreshed index confirms its hash"
    );
}

#[test]
fn interrupted_pack_download_resumes_with_http_range() {
    use std::io::{BufRead as _, BufReader, Write as _};
    let _turn = one_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let store = store_in(&dir);
    let (m, _, row, tar) = pack();
    store.save_index(&index_of(vec![row.clone()])).unwrap();

    let cut = tar.len() / 2;
    let ranges = std::sync::Arc::new(Mutex::new(Vec::<Option<String>>::new()));
    let seen = ranges.clone();
    let body = tar.clone();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for (i, stream) in listener.incoming().flatten().enumerate() {
            let mut reader = BufReader::new(stream);
            let mut range = None;
            loop {
                let mut h = String::new();
                if reader.read_line(&mut h).is_err() || h.trim().is_empty() {
                    break;
                }
                if let Some(v) = h.to_ascii_lowercase().strip_prefix("range:") {
                    range = Some(v.trim().to_owned());
                }
            }
            seen.lock().unwrap().push(range.clone());
            let mut stream = reader.into_inner();
            if i == 0 {
                // Advertise full Content-Length, then drop the socket halfway.
                let head =
                    format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = stream.write_all(head.as_bytes()).and_then(|()| stream.write_all(&body[..cut]));
            } else {
                let rest = &body[cut..];
                let head = format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {cut}-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len() - 1,
                    body.len(),
                    rest.len()
                );
                let _ = stream.write_all(head.as_bytes()).and_then(|()| stream.write_all(rest));
                break;
            }
        }
    });

    assert!(download_from(&store, Some(&url), &row.id).unwrap_err().starts_with("network:"));
    let (part, _) = store.part_paths(&row.unit);
    assert_eq!(fs::metadata(&part).unwrap().len(), cut as u64, "kept partial download");

    let installed = download_from(&store, Some(&url), &row.id).unwrap();
    assert_eq!(installed, m);
    assert!(!part.exists(), "cleaned up after install");
    assert_eq!(*ranges.lock().unwrap(), [None, Some(format!("bytes={cut}-"))]);
}

#[test]
fn downloads_that_cannot_happen_are_refused_plainly() {
    let _turn = one_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let store = store_in(&dir);
    let (_, _, row, tar) = pack();
    let (url, _) = release(vec![row.clone()], &[(&row.file, &tar)]);
    // A build without a packs address.
    assert_eq!(download_from(&store, None, &row.id).unwrap_err(), NOT_CONFIGURED);
    // A unit the index doesn't list.
    refresh_index_from(&store, &url).unwrap();
    assert_eq!(download_from(&store, Some(&url), "id/nope").unwrap_err(), "no such pack");
    // Listed, but its audio isn't bundled yet.
    let waiting = PackSummary { file_sha256: None, ..row.clone() };
    let (url2, log) = release(vec![waiting], &[]);
    refresh_index_from(&store, &url2).unwrap();
    assert_eq!(
        download_from(&store, Some(&url2), &row.id).unwrap_err(),
        "this unit's audio isn't published yet"
    );
    assert_eq!(*log.lock().unwrap(), ["index.json"], "no tar asked for");
    // Announced but missing from the release.
    let (url3, _) = release(vec![row.clone()], &[]);
    refresh_index_from(&store, &url3).unwrap();
    let err = download_from(&store, Some(&url3), &row.id).unwrap_err();
    assert!(err.ends_with("HTTP 404"), "{err}");
    // The same pack already on its way (a prefetch and a tap).
    IN_FLIGHT.lock().unwrap().insert(row.id.clone());
    assert_eq!(jobs().get(&row.id), Some(&Job::Downloading), "shown as downloading");
    assert_eq!(download_from(&store, Some(&url), &row.id).unwrap_err(), ALREADY_DOWNLOADING);
    IN_FLIGHT.lock().unwrap().remove(&row.id);
    assert!(store.installed().is_empty());
}

#[test]
fn opening_a_lesson_fetches_its_unit_and_updates_bring_new_versions() {
    let _turn = one_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let store = store_in(&dir);
    let (_, _, row, tar) = pack();
    let (url, log) = release(vec![row.clone()], &[(&row.file, &tar)]);
    prefetch_from(&store, &url, "bipa1.u01.l02");
    assert_eq!(store.installed_versions().get(&row.id), Some(&1));
    assert_eq!(*log.lock().unwrap(), ["index.json".to_owned(), row.file.clone()]);
    // Installed and current: nothing more to fetch or update.
    prefetch_from(&store, &url, "bipa1.u01.l03");
    assert_eq!(log.lock().unwrap().len(), 3, "only the index again");
    assert_eq!(update_installed_from(&store, &url), 0);

    // Version 2 on the release: the update re-downloads it.
    let (_, _, row2, tar2) = pack_v(2);
    let (url2, _) = release(vec![row2.clone()], &[(&row2.file, &tar2)]);
    assert_eq!(update_installed_from(&store, &url2), 1);
    assert_eq!(store.installed_versions().get(&row.id), Some(&2));
    // An index that can't be read updates nothing.
    let (gone, _) = serve(BTreeMap::new());
    assert_eq!(update_installed_from(&store, &gone), 0);
    // A broken tar on the release: counted as not updated, old pack kept.
    let (_, _, row3, _) = pack_v(3);
    let (url3, _) = release(vec![row3.clone()], &[(&row3.file, b"not a tar")]);
    assert_eq!(update_installed_from(&store, &url3), 0);
    assert_eq!(store.installed_versions().get(&row.id), Some(&2));
    // With no index at all, prefetch falls back to the one it has.
    prefetch_from(&store, &gone, "bipa1.u01.l01");
    assert_eq!(store.installed_versions().get(&row.id), Some(&2));
}

#[test]
fn packs_with_odd_contents_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_in(&dir);
    let (m, clips, row, _) = pack();
    let signed = |tar: Vec<u8>| (PackSummary { file_sha256: Some(sha256_hex(&tar)), ..row.clone() }, tar);
    // No manifest in the tar.
    let mut b = tar::Builder::new(Vec::new());
    let mut h = tar::Header::new_ustar();
    h.set_size(1);
    h.set_mode(0o644);
    b.append_data(&mut h, "readme.txt", &b"x"[..]).unwrap();
    let (r, t) = signed(b.into_inner().unwrap());
    assert_eq!(store.install(&r, &t).unwrap_err(), "the pack has no manifest");
    // A clip the manifest doesn't list.
    let tar = {
        let mut b = tar::Builder::new(Vec::new());
        let mut add = |name: &str, data: &[u8]| {
            let mut h = tar::Header::new_ustar();
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            b.append_data(&mut h, name, data).unwrap();
        };
        add("manifest.json", &serde_json::to_vec(&m).unwrap());
        add("audio/stray.mp3", b"stray");
        b.into_inner().unwrap()
    };
    let (r, t) = signed(tar);
    assert!(store.install(&r, &t).unwrap_err().contains("isn't in the manifest"));
    // A clip the manifest lists but the tar lacks.
    let short: Vec<Vec<u8>> = clips[..clips.len() - 1].to_vec();
    let (r, t) = signed(tar_of(&m, &short));
    assert!(store.install(&r, &t).unwrap_err().ends_with("is missing"));
    assert!(store.installed().is_empty());
}

#[test]
fn removing_one_pack_keeps_clips_another_still_needs() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_in(&dir);
    let (m, _, row, tar) = pack();
    store.install(&row, &tar).unwrap();
    // A second unit's manifest sharing the first clip.
    let mut other = m.clone();
    other.id = "id/bipa1.u02".into();
    other.unit = "bipa1.u02".into();
    other.audio.truncate(1);
    fs::write(store.manifest_path("bipa1.u02"), serde_json::to_vec(&other).unwrap()).unwrap();
    store.remove("bipa1.u01").unwrap();
    assert!(store.clip(m.audio[0].voice, &m.audio[0].text).is_some(), "still needed by bipa1.u02");
    assert!(m.audio[1..].iter().all(|a| store.clip(a.voice, &a.text).is_none()));
    // Removing what isn't there is fine.
    store.remove("bipa9.u09").unwrap();
    // The release address: plain https by default.
    if std::env::var_os("KABAR_PACKS_URL").is_none() && option_env!("KABAR_PACKS_URL").is_none() {
        assert_eq!(base_url().as_deref(), Some(DEFAULT_PACKS_URL));
    }
}

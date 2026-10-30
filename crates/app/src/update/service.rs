//! Update orchestration behind the existing Updater API.

use super::*;

impl Updater {
    pub(super) fn new(
        dir: PathBuf,
        info: BuildInfo,
        platform: Option<UpdatePlatform>,
        feed_override: Option<String>,
        off: Option<Off>,
    ) -> Self {
        let prefs = load_prefs(&dir);
        let off = off.or(if platform.is_none() {
            Some(Off::NoPlatform)
        } else if info.key.is_none() {
            Some(Off::NoKey)
        } else {
            None
        });
        let mut prefs = prefs;
        let in_use = dir.parent().and_then(|d| content::in_use(d, &info));
        let content_build = in_use.as_ref().map_or(0, |c| c.build);
        let content_since_ms = in_use.and_then(|c| c.fetched_ms);
        let this = format!("{}+{}", info.version, info.build.max(content_build));
        if off.is_none() && prefs.last_run.as_deref() != Some(this.as_str()) {
            // Updated (not a first install): say what's new once.
            if let Some(prev) = prefs.last_run.take() {
                prefs.whats_new_from = Some(prev);
            }
            prefs.last_run = Some(this);
            if let Err(e) = save_prefs(&dir, &prefs) {
                crate::diag::report!("update prefs: {e}");
            }
        }
        let history = changelog::load(&dir);
        let installed = info.installed(prefs.channel);
        let mut phase = Phase::Idle;
        if off.is_none()
            && let Some(offer) = load_staged(&dir)
            && is_newer(&installed, &offer)
            && dir.join(&offer.asset.file).metadata().is_ok_and(|m| m.len() == offer.asset.size)
        {
            phase = Phase::Ready(offer);
        }
        let status = Status {
            off,
            phase,
            channel: installed.channel,
            preview: info.preview,
            prefs,
            version: info.version.to_string(),
            build: info.build,
            content_build,
            content_since_ms,
            content_waiting: None,
            content_check: None,
            packs_refreshed: 0,
            checking_all: false,
            history,
        };
        let (tx, _) = watch::channel(status);
        Self {
            dir,
            info,
            platform,
            feed_override,
            retry_after: Duration::from_secs(20),
            tx,
            job: Mutex::new(()),
        }
    }

    /// What's installed, on the channel chosen now.
    pub(super) fn installed(&self) -> Installed {
        self.info.installed(self.tx.borrow().prefs.channel)
    }

    /// The feed folder: the local override in development, else the
    /// channel's GitHub release.
    pub(super) fn feed_url(&self) -> String {
        match &self.feed_override {
            Some(url) => url.trim_end_matches('/').to_owned(),
            None => format!("{FEEDS_URL}/{}", self.installed().channel.release_tag()),
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<Status> {
        self.tx.subscribe()
    }

    pub fn status(&self) -> Status {
        self.tx.borrow().clone()
    }

    pub(super) fn set_phase(&self, phase: Phase) {
        self.tx.send_modify(|s| s.phase = phase);
    }

    pub(super) fn update_prefs(&self, f: impl FnOnce(&mut Prefs)) {
        self.tx.send_modify(|s| {
            f(&mut s.prefs);
            s.channel = self.info.channel(s.prefs.channel);
            if let Err(e) = save_prefs(&self.dir, &s.prefs) {
                crate::diag::report!("update prefs: {e}");
            }
        });
    }

    /// The background loop's step: check when due, then refresh outdated
    /// offline packs, then announce what it found.
    pub(super) fn tick(&'static self) {
        if self.step(CHECK_EVERY, crate::platform::metered_network()) {
            crate::packs::update_installed(false);
        }
        self.send_notice(None, crate::platform::update_notice::post);
    }

    /// Checks when the last check is `every` old (true if it did), or starts
    /// the download an earlier check left waiting for Wi-Fi.
    pub(super) fn step(&'static self, every: Duration, metered: bool) -> bool {
        let s = self.status();
        if s.off.is_some() || s.prefs.mode == Mode::Off {
            return false;
        }
        let due =
            s.prefs.last_check_ms.is_none_or(|t| now_ms().saturating_sub(t) >= every.as_millis() as u64);
        // Automatic downloads wait for Wi-Fi (a ~45 MB APK on mobile data,
        // up to every merge on the preview channel); the offer still shows.
        let auto = s.prefs.mode == Mode::Auto && !metered;
        if due {
            // Content bundles (~8 MB) wait for Wi-Fi too.
            self.check_with(auto, !metered);
        } else if auto && let Phase::Available(_) = s.phase {
            self.download();
        }
        due
    }

    /// Sends the notification due now through `post` (false: not shown, try
    /// again at the next check) and remembers it once shown.
    pub(super) fn send_notice(
        &self,
        local: Option<LocalTime>,
        post: impl FnOnce(&Notice) -> bool,
    ) -> Option<Notice> {
        let n = notice(&self.status(), local, now_ms())?;
        if !post(&n) {
            return None;
        }
        self.update_prefs(|p| p.notified = Some(n.key.clone()));
        Some(n)
    }

    /// Android's background job: the same step as the app's loop, then the
    /// notification to post, if `can_notify` (notifications allowed, Kabar
    /// not in front). Not marked as sent: one Android doesn't show is tried
    /// again at the next run ([`Self::shown`] marks it).
    pub(super) fn background(
        &'static self,
        metered: bool,
        can_notify: bool,
        local: LocalTime,
    ) -> Option<Notice> {
        self.step(BACKGROUND_EVERY, metered);
        if !can_notify {
            return None;
        }
        notice(&self.status(), Some(local), now_ms())
    }

    /// The notification `key` was shown.
    pub(super) fn shown(&self, key: &str) {
        if self.status().prefs.notified.as_deref() != Some(key) {
            self.update_prefs(|p| p.notified = Some(key.to_owned()));
        }
    }

    /// Whether Android's background job should run: updates work here and
    /// aren't switched off.
    pub fn background_checks(&self) -> bool {
        let s = self.status();
        s.off.is_none() && s.prefs.mode != Mode::Off
    }

    /// Keeps the language and quiet hours the background job words and
    /// times its notification by.
    pub fn set_notice_context(&self, ctx: NoticeContext) {
        if self.status().prefs.notice != Some(ctx) {
            self.update_prefs(|p| p.notice = Some(ctx));
        }
    }

    pub fn set_notify(&self, on: bool) {
        self.update_prefs(|p| p.notify = on);
        if !on {
            crate::platform::update_notice::clear();
        }
    }

    /// Looks at the feed; with `download`, fetches a newer build right away.
    pub fn check(&'static self, download: bool) {
        self.check_with(download, true);
    }

    /// "Check for updates" by hand: the app, lessons and look, and the
    /// offline audio packs already on this device, in one go.
    pub fn check_everything(&'static self) {
        let mut started = false;
        self.tx.send_if_modified(|s| {
            started = !s.checking_all;
            if started {
                s.checking_all = true;
                s.packs_refreshed = 0;
            }
            started
        });
        if !started {
            return;
        }
        self.check(false);
        let reached = !matches!(self.status().phase, Phase::Failed { offer: None, .. });
        let n = if self.status().off.is_none() && reached { crate::packs::update_installed(true) } else { 0 };
        self.tx.send_modify(|s| {
            s.packs_refreshed = n;
            s.checking_all = false;
        });
    }

    /// [`Self::check`]; with `content`, a newer content bundle is fetched too.
    pub(super) fn check_with(&'static self, download: bool, content: bool) {
        let Ok(_job) = self.job.try_lock() else { return };
        if self.status().off.is_some() {
            return;
        }
        let keep = match self.status().phase {
            // A finished download for the same offer stays; it's re-checked anyway.
            Phase::Ready(o) | Phase::Installed(o) | Phase::NeedsPermission(o) => Some(o),
            _ => None,
        };
        self.set_phase(Phase::Checking);
        let result = self.fetch_offer(content);
        self.update_prefs(|p| p.last_check_ms = Some(now_ms()));
        match result {
            Ok(Some(offer)) => {
                if let Some(k) = keep.filter(|k| k.asset == offer.asset) {
                    self.set_phase(match self.status().phase {
                        Phase::Checking => Phase::Ready(k),
                        other => other,
                    });
                    return;
                }
                self.set_phase(Phase::Available(offer.clone()));
                if download {
                    self.download_locked(offer);
                }
            }
            Ok(None) => {}
            Err(why) => self.set_phase(Phase::Failed { offer: None, why }),
        }
    }

    /// `Ok(None)` after setting UpToDate / NotForThisDevice.
    pub(super) fn fetch_offer(&self, content: bool) -> Result<Option<Offer>, Failure> {
        let key = self.info.key.ok_or(Failure::Verify("no key".into()))?;
        let base = self.feed_url();
        let fetch = || -> Result<_, Failure> {
            let json = get_small(&format!("{base}/{MANIFEST_FILE}"))?;
            let sig = get_small(&format!("{base}/{SIGNATURE_FILE}"))?;
            let sig = String::from_utf8(sig).map_err(|_| Failure::Verify("signature".into()))?;
            Ok(verify_manifest(&json, &sig, &key))
        };
        // A feed caught mid-upload pairs a new signature with the old
        // manifest: look once more before calling it a failure.
        let m = match fetch()? {
            Err(UpdateError::NotSigned) => {
                std::thread::sleep(self.retry_after);
                fetch()?
            }
            first => first,
        };
        let m = m.map_err(|e| match e {
            // A preview whose CI key was renewed can't verify any newer feed.
            UpdateError::NotSigned if self.info.preview => Failure::KeyChanged,
            other => update_failure(other),
        })?;
        self.remember_history(&m.history);
        if content && let Some(data) = self.dir.parent() {
            let found = content::refresh(data, &self.installed(), &self.info, &m, &self.feed_url());
            self.tx.send_modify(|s| {
                s.content_check = Some(found);
                if let content::Refresh::Taken(build) = found {
                    s.content_waiting = Some(build);
                }
            });
        }
        let platform = self.platform.ok_or(Failure::Verify("no platform".into()))?;
        match decide(&self.installed(), &m, platform).map_err(update_failure)? {
            Decision::UpToDate => {
                self.set_phase(Phase::UpToDate);
                self.clear_downloads(None);
                Ok(None)
            }
            Decision::NotForThisPlatform => {
                self.set_phase(Phase::NotForThisDevice);
                Ok(None)
            }
            Decision::Available(asset) => {
                if asset.size > MAX_BUILD_BYTES {
                    return Err(Failure::Verify("build too large".into()));
                }
                Ok(Some(Offer::from_manifest(&m, asset, &self.installed())))
            }
        }
    }

    /// Keeps a verified feed's history, and lets a pending "What's new" go
    /// when the feed has nothing to say about this build.
    pub(super) fn remember_history(&self, fresh: &[BuildChanges]) {
        let known = self.status().history;
        if let Some(all) = changelog::merge(&known, fresh) {
            if let Err(e) = write_json(&self.dir, changelog::FILE, &all) {
                crate::diag::report!("update history: {e}");
            }
            self.tx.send_modify(|s| s.history = all);
        }
        let s = self.status();
        if s.prefs.whats_new_from.is_some() && s.whats_new().is_empty() {
            self.update_prefs(|p| p.whats_new_from = None);
        }
    }

    /// "What's new" was seen (closed).
    pub fn close_whats_new(&self) {
        if self.status().prefs.whats_new_from.is_some() {
            self.update_prefs(|p| p.whats_new_from = None);
        }
    }

    /// Downloads the offered build (a tap in Notify mode).
    pub fn download(&'static self) {
        let Ok(_job) = self.job.try_lock() else { return };
        if let Phase::Available(offer) | Phase::Failed { offer: Some(offer), .. } = self.status().phase {
            self.download_locked(offer);
        }
    }

    pub(super) fn download_locked(&self, offer: Offer) {
        let base = self.feed_url();
        let _ = fs::create_dir_all(&self.dir);
        self.clear_downloads(Some(&offer.asset.file));
        let path = self.dir.join(&offer.asset.file);
        self.set_phase(Phase::Downloading { offer: offer.clone(), done: 0 });
        let url = format!("{base}/{}", offer.asset.file);
        let mut result = self.fetch_build(&url, &offer, &path);
        // A connection that dropped mid-file is picked up again where it
        // stopped, a few times, before giving up until the next try.
        for wait in DOWNLOAD_RETRIES {
            match &result {
                Err(Fetch { why: Failure::Network, progressed: true }) => {
                    std::thread::sleep(self.retry_after * wait);
                    result = self.fetch_build(&url, &offer, &path);
                }
                _ => break,
            }
        }
        match result.map_err(|f| f.why) {
            Ok(()) => {
                if let Err(e) = save_staged(&self.dir, &offer) {
                    crate::diag::report!("update staged.json: {e}");
                }
                self.set_phase(Phase::Ready(offer));
            }
            Err(why) => {
                let _ = fs::remove_file(&path);
                self.set_phase(Phase::Failed { offer: Some(offer), why });
            }
        }
    }

    /// Streams the build to `<file>.part` while hashing it, checks size and
    /// SHA-256 against the signed manifest, then renames it into place. A
    /// part left by an interrupted try of the same build is continued with
    /// an HTTP Range request instead of starting over; the hash still covers
    /// every byte, old and new.
    pub(super) fn fetch_build(&self, url: &str, offer: &Offer, path: &Path) -> Result<(), Fetch> {
        let (part, meta) = part_paths(path);
        let want = PartMeta { size: offer.asset.size, sha256: offer.asset.sha256.clone() };
        let on_disk = fs::metadata(&part).map_or(0, |m| m.len());
        let mut have = match read_part_meta(&meta) {
            Some(m) if m == want && on_disk < want.size => on_disk,
            _ => 0,
        };
        if have == 0 {
            let _ = fs::remove_file(&part);
            let json = serde_json::to_vec(&want).unwrap_or_default();
            fs::write(&meta, json).map_err(|e| Fetch::stopped(Failure::Install(e.to_string())))?;
        }
        let mut req = agent(Duration::from_secs(30 * 60)).get(url);
        if have > 0 {
            req = req.header("Range", format!("bytes={have}-"));
        }
        let mut res = req.call().map_err(|_| Fetch::stopped(Failure::Network))?;
        match res.status().as_u16() {
            206 if have > 0 && range_start(&res) == Some(have) => {}
            // The server sent the whole file (or ignored the range): start over.
            200 => have = 0,
            // The range no longer fits this file: drop the part, next try starts fresh.
            416 => {
                let _ = fs::remove_file(&part);
                return Err(Fetch::stopped(Failure::Network));
            }
            _ => return Err(Fetch::stopped(Failure::Network)),
        }
        let mut hash = Sha256::new();
        let mut file = if have > 0 {
            let mut old =
                fs::File::open(&part).map_err(|e| Fetch::stopped(Failure::Install(e.to_string())))?;
            let read = std::io::copy(&mut (&mut old).take(have), &mut hash)
                .map_err(|e| Fetch::stopped(Failure::Install(e.to_string())))?;
            if read != have {
                let _ = fs::remove_file(&part);
                return Err(Fetch::stopped(Failure::Network));
            }
            fs::OpenOptions::new().append(true).open(&part)
        } else {
            fs::File::create(&part)
        }
        .map_err(|e| Fetch::stopped(Failure::Install(e.to_string())))?;
        let mut reader = res.body_mut().as_reader().take(offer.asset.size - have + 1);
        let (mut done, mut shown) = (have, have);
        self.set_phase(Phase::Downloading { offer: offer.clone(), done });
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let progressed = done > have;
            let n = reader.read(&mut buf).map_err(|_| Fetch { why: Failure::Network, progressed })?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n]);
            file.write_all(&buf[..n]).map_err(|e| Fetch::stopped(Failure::Install(e.to_string())))?;
            done += n as u64;
            if done - shown > offer.asset.size / 100 {
                shown = done;
                self.set_phase(Phase::Downloading { offer: offer.clone(), done });
            }
        }
        file.sync_all().map_err(|e| Fetch::stopped(Failure::Install(e.to_string())))?;
        drop(file);
        if done < offer.asset.size {
            // The body ended early: keep what came, the next try continues it.
            return Err(Fetch { why: Failure::Network, progressed: done > have });
        }
        let hex: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if let Err(e) = check_download(&offer.asset, done, &hex) {
            let _ = fs::remove_file(&part);
            let _ = fs::remove_file(&meta);
            if have > 0 {
                // The kept part was likely damaged: fetch it whole once more
                // (a retry from zero) before calling the build bad.
                return Err(Fetch { why: Failure::Network, progressed: true });
            }
            return Err(Fetch::stopped(update_failure(e)));
        }
        fs::rename(&part, path).map_err(|e| Fetch::stopped(Failure::Install(e.to_string())))?;
        let _ = fs::remove_file(&meta);
        Ok(())
    }

    /// Re-hashes the staged file: whatever sat on disk since the download
    /// is checked again before it's installed.
    pub(super) fn verified_staged(&self, offer: &Offer) -> Result<PathBuf, Failure> {
        let path = self.dir.join(&offer.asset.file);
        let mut file = fs::File::open(&path).map_err(|_| Failure::Verify("download missing".into()))?;
        let mut hash = Sha256::new();
        let size = std::io::copy(&mut file, &mut hash).map_err(|e| Failure::Verify(e.to_string()))?;
        let hex: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        check_download(&offer.asset, size, &hex).map_err(update_failure)?;
        Ok(path)
    }

    /// Installs the ready download (a tap, or calm moment in Auto mode).
    pub fn install(&'static self, texts: InstallTexts) {
        let Ok(_job) = self.job.try_lock() else { return };
        let offer = match self.status().phase {
            Phase::Ready(o)
            | Phase::NeedsPermission(o)
            | Phase::Failed { offer: Some(o), why: Failure::Install(_) } => o,
            _ => return,
        };
        let path = match self.verified_staged(&offer) {
            Ok(p) => p,
            Err(why) => {
                self.clear_downloads(None);
                self.set_phase(Phase::Failed { offer: Some(offer), why });
                return;
            }
        };
        self.set_phase(Phase::Installing(offer.clone()));
        // "Kabar was updated" says what's new when the feed says it.
        let mut texts = texts;
        let whats_new = notes_in(texts.lang, &offer.notes);
        if !whats_new.is_empty() {
            texts.done_body = whats_new;
        }
        let next = install_build(&path, &offer, &texts);
        if let Phase::Installed(_) = next {
            self.clear_downloads(None);
        }
        self.set_phase(next);
    }

    pub fn set_mode(&self, mode: Mode) {
        self.update_prefs(|p| p.mode = mode);
        crate::platform::update_notice::schedule(self.background_checks());
    }

    /// Switches release channel and looks again.
    pub fn set_channel(&'static self, channel: Channel) {
        if self.status().preview || channel == Channel::Preview {
            return;
        }
        self.update_prefs(|p| p.channel = Some(channel));
        self.clear_downloads(None);
        self.set_phase(Phase::Idle);
        let auto = self.status().prefs.mode == Mode::Auto;
        std::thread::spawn(move || self.check(auto));
    }

    pub fn dismiss(&self) {
        let key = self.status().banner().map(Offer::key);
        self.update_prefs(|p| p.dismissed = key);
        crate::platform::update_notice::clear();
    }

    /// Android: polls PackageInstaller after an install started.
    #[cfg(target_os = "android")]
    pub fn poll_install(&self) {
        use crate::platform::android::{InstallState, install_state};
        let Phase::Installing(offer) = self.status().phase else { return };
        match install_state() {
            // A success ends this process: the new Kabar starts in its place.
            Ok(InstallState::Working | InstallState::WaitingUser) => {}
            Ok(InstallState::Idle | InstallState::Cancelled) => self.set_phase(Phase::Ready(offer)),
            Ok(InstallState::Failed(why)) | Err(why) => {
                self.update_prefs(|p| p.auto_failed = Some(offer.key()));
                self.set_phase(Phase::Failed { offer: Some(offer), why: Failure::Install(why) });
            }
        }
    }

    /// Removes every download except `keep`, and the staged record unless kept.
    pub(super) fn clear_downloads(&self, keep: Option<&str>) {
        let Ok(entries) = fs::read_dir(&self.dir) else { return };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name == "prefs.json" || name == changelog::FILE || Some(name.as_str()) == keep {
                continue;
            }
            // The unfinished download of the kept build, to resume.
            if let Some(k) = keep {
                let (part, meta) = part_paths(Path::new(k));
                if Path::new(&name) == part || Path::new(&name) == meta {
                    continue;
                }
            }
            if name == "staged.json" && keep.is_some() {
                continue;
            }
            let _ = fs::remove_file(e.path());
        }
    }
}

//! TTS proxy: Azure Speech neural voices over REST. The server builds the
//! SSML from plain text, so a device can't inject markup, and counts the
//! characters against the free monthly quota.

use kabar_protocol::cloud::{ErrorCode, TtsRequest};

use crate::tokens::sha256_hex;

pub const OUTPUT_FORMAT: &str = "audio-24khz-48kbitrate-mono-mp3";
pub const MAX_SEGMENTS: usize = 40;
pub const MAX_CHARS: usize = 5_000;

pub fn azure_url(region: &str) -> String {
    format!("https://{region}.tts.speech.microsoft.com/cognitiveservices/v1")
}

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c.is_control() && c != '\n' => {}
            c => out.push(c),
        }
    }
    out
}

/// Billable characters (Azure counts the spoken text, not the SSML tags).
pub fn billable_chars(req: &TtsRequest) -> u64 {
    req.segments.iter().map(|s| s.text.chars().count() as u64).sum()
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct Invalid(pub String);

impl Invalid {
    pub fn code(&self) -> ErrorCode {
        ErrorCode::InvalidRequest
    }
}

pub fn validate(req: &TtsRequest) -> Result<(), Invalid> {
    let invalid = |m: String| Err(Invalid(m));
    if req.segments.is_empty() || req.segments.len() > MAX_SEGMENTS {
        return invalid(format!("1..={MAX_SEGMENTS} segments"));
    }
    if req.segments.iter().any(|s| s.text.trim().is_empty()) {
        return invalid("empty segment".into());
    }
    if billable_chars(req) > MAX_CHARS as u64 {
        return invalid(format!("at most {MAX_CHARS} characters per request"));
    }
    if !(-50..=50).contains(&req.rate_percent) {
        return invalid("rate_percent must be -50..=50".into());
    }
    Ok(())
}

pub fn ssml(req: &TtsRequest) -> String {
    let lang = req.segments.first().map_or("id-ID", |s| s.voice.locale());
    let mut out =
        format!(r#"<speak version="1.0" xmlns="http://www.w3.org/2001/10/synthesis" xml:lang="{lang}">"#);
    for seg in &req.segments {
        let text = escape_xml(seg.text.trim());
        out.push_str(&format!(r#"<voice name="{}">"#, seg.voice.azure_name()));
        if req.rate_percent == 0 {
            out.push_str(&text);
        } else {
            out.push_str(&format!(r#"<prosody rate="{:+}%">{text}</prosody>"#, req.rate_percent));
        }
        out.push_str("</voice>");
    }
    out.push_str("</speak>");
    out
}

/// Cache key for the audio (same SSML and format → same bytes, forever).
pub fn cache_key(ssml: &str) -> String {
    sha256_hex(format!("{OUTPUT_FORMAT}\n{ssml}").as_bytes())
}

#[cfg(test)]
mod tests {
    use kabar_protocol::cloud::{TtsSegment, Voice};

    use super::*;

    fn req(segments: &[(Voice, &str)], rate: i8) -> TtsRequest {
        TtsRequest {
            segments: segments.iter().map(|(v, t)| TtsSegment { voice: *v, text: (*t).into() }).collect(),
            rate_percent: rate,
        }
    }

    #[test]
    fn single_voice() {
        let r = req(&[(Voice::IdGadis, "Apa kabar?")], 0);
        assert_eq!(
            ssml(&r),
            r#"<speak version="1.0" xmlns="http://www.w3.org/2001/10/synthesis" xml:lang="id-ID"><voice name="id-ID-GadisNeural">Apa kabar?</voice></speak>"#
        );
        assert_eq!(billable_chars(&r), 10);
    }

    #[test]
    fn dialogue_with_slow_rate() {
        let r = req(&[(Voice::JvSiti, "Kowe wis mangan?"), (Voice::JvDimas, "Sampeyan sampun nedha?")], -20);
        let s = ssml(&r);
        assert!(s.contains(r#"xml:lang="jv-ID""#));
        assert!(s.contains(
            r#"<voice name="jv-ID-SitiNeural"><prosody rate="-20%">Kowe wis mangan?</prosody></voice>"#
        ));
        assert!(s.contains(r#"<voice name="jv-ID-DimasNeural">"#));
        assert_ne!(cache_key(&s), cache_key(&ssml(&req(&[(Voice::JvSiti, "Kowe wis mangan?")], -20))));
    }

    #[test]
    fn markup_is_escaped() {
        let s = ssml(&req(&[(Voice::IdArdi, "<break time=\"9s\"/> & ok")], 10));
        assert!(s.contains("&lt;break time=&quot;9s&quot;/&gt; &amp; ok"));
        assert!(s.contains(r#"rate="+10%""#));
    }

    #[test]
    fn limits() {
        assert!(validate(&req(&[], 0)).is_err());
        assert!(validate(&req(&[(Voice::IdGadis, "  ")], 0)).is_err());
        assert!(validate(&req(&[(Voice::IdGadis, "ok")], 60)).is_err());
        let long = "a".repeat(MAX_CHARS + 1);
        assert!(validate(&req(&[(Voice::IdGadis, &long)], 0)).is_err());
        assert!(validate(&req(&[(Voice::IdGadis, "Apa kabar?")], -50)).is_ok());
    }
}

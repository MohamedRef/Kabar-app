//! The version people read (docs/RELEASE.md §1). Since 2026-11-08 (Mohamed)
//! a release is `YEAR.MONTH.N` (`2026.11.1`) and every build, preview or
//! release, carries one rising build number.

/// The workspace version this build was made from: the release's own for a
/// release build, the latest release's for a preview (`scripts/version.sh`).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The build number CI baked in (`KABAR_BUILD`); 0 for a local build.
pub fn build() -> u64 {
    parse_build(option_env!("KABAR_BUILD"))
}

fn parse_build(raw: Option<&str>) -> u64 {
    raw.and_then(|b| b.trim().parse().ok()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_build_number_is_read_leniently() {
        assert_eq!(parse_build(Some(" 812\n")), 812);
        assert_eq!(parse_build(Some("x")), 0);
        assert_eq!(parse_build(None), 0);
    }
}

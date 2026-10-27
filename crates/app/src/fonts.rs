//! Bundled UI fonts (docs/VISUAL-ASSETS.md, prompt 8), all SIL OFL 1.1 from
//! the Fontsource 5.3.0 npm packages: Plus Jakarta Sans (Latin UI and the
//! target languages), IBM Plex Sans Arabic (Arabic UI) and Amiri (the
//! خبر → kabar reveal). Embedded as `data:` URLs like the Naskh font, so
//! every device shows the same faces.

use base64::Engine as _;

macro_rules! font {
    ($path:literal) => {
        include_bytes!(concat!("../../../assets/fonts/", $path))
    };
}

/// Latin basic and extended ranges, as Fontsource splits them.
const LATIN: &str = "U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD";
const LATIN_EXT: &str = "U+0100-02BA,U+02BD-02C5,U+02C7-02CC,U+02CE-02D7,U+02DD-02FF,U+0304,U+0308,U+0329,U+1D00-1DBF,U+1E00-1E9F,U+1EF2-1EFF,U+2020,U+20A0-20AB,U+20AD-20C0,U+2113,U+2C60-2C7F,U+A720-A7FF";
const ARABIC: &str = "U+0600-06FF,U+0750-077F,U+0870-088E,U+0890-0891,U+0897-08E1,U+08E3-08FF,U+200C-200E,U+2010-2011,U+204F,U+2E41,U+FB50-FDFF,U+FE70-FE74,U+FE76-FEFC";

/// (family, weight, unicode-range, woff2 bytes)
const FACES: [(&str, &str, &str, &[u8]); 6] = [
    (
        "Plus Jakarta Sans",
        "200 800",
        LATIN,
        font!("plus-jakarta-sans/plus-jakarta-sans-latin-wght-normal.woff2"),
    ),
    (
        "Plus Jakarta Sans",
        "200 800",
        LATIN_EXT,
        font!("plus-jakarta-sans/plus-jakarta-sans-latin-ext-wght-normal.woff2"),
    ),
    (
        "IBM Plex Sans Arabic",
        "400",
        ARABIC,
        font!("ibm-plex-sans-arabic/ibm-plex-sans-arabic-arabic-400-normal.woff2"),
    ),
    (
        "IBM Plex Sans Arabic",
        "600",
        ARABIC,
        font!("ibm-plex-sans-arabic/ibm-plex-sans-arabic-arabic-600-normal.woff2"),
    ),
    (
        "IBM Plex Sans Arabic",
        "700",
        ARABIC,
        font!("ibm-plex-sans-arabic/ibm-plex-sans-arabic-arabic-700-normal.woff2"),
    ),
    ("Amiri", "400", ARABIC, font!("amiri/amiri-arabic-400-normal.woff2")),
];

pub fn css() -> String {
    FACES
        .iter()
        .map(|(family, weight, range, bytes)| {
            let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
            format!(
                "@font-face {{ font-family: \"{family}\"; font-style: normal; font-display: swap; font-weight: {weight}; \
                 src: url(data:font/woff2;base64,{b64}) format(\"woff2\"); unicode-range: {range}; }}\n"
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_face_is_woff2() {
        for (family, _, _, bytes) in super::FACES {
            assert_eq!(&bytes[..4], b"wOF2", "{family}");
        }
        assert_eq!(super::css().matches("@font-face").count(), 6);
    }
}

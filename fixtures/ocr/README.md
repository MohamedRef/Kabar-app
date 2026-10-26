# OCR fixtures

Engine output for `crates/app/src/platform/ocr.rs` tests. Tests never run a real
OCR engine. Name: `<case>.<engine>.json`.

`whatsapp_javanese.ml_kit.json` is hand-written in the exact JSON shape that
`android/src/main/kotlin/app/kabar/OcrBridge.kt` emits (a 1440×3120 S24 Ultra
screenshot of the §24 WhatsApp chat), including the rough edges a real capture
has: a line without confidence, a whitespace-only line, a block with an empty
box, an empty block. Replace it with a capture recorded on the phone and keep
those edge cases.

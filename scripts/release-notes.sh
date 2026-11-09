#!/usr/bin/env bash
# The GitHub release page for a Kabar release (docs/RELEASE.md §4), in
# Egyptian Arabic (Mohamed, 2026-11-09): what's new from the signed feed's
# newest history entry when it is this version's (the changes/ lines merged
# since the last release; a release without any says so),
# how to install, and the checksums. A line without Arabic text is left out.
#
#   scripts/release-notes.sh FEED/update.json CHANNEL [NOTES_EN] [NOTES_AR_EG]
#
# Prints Markdown. The version, build and commit come from the manifest.
# NOTES_EN is accepted for the workflow's sake but not shown.
set -euo pipefail

manifest="${1:?usage: scripts/release-notes.sh FEED/update.json CHANNEL [notes-en] [notes-ar-eg]}"
channel="${2:?channel}"
notes_ar_eg="${4:-}"
v="$(jq -r .version "$manifest")"
build="$(jq -r .build "$manifest")"
commit="$(jq -r '.commit // ""' "$manifest")"
case "$channel" in
  alpha) channel_ar="ألفا" ;;
  beta) channel_ar="بيتا" ;;
  stable) channel_ar="المستقرة" ;;
  *) channel_ar="$channel" ;;
esac

echo "**Kabar $v** · بناء $build${commit:+ · \`$commit\`} · قناة $channel_ar"
echo
if [ -n "$notes_ar_eg" ]; then
  echo "$notes_ar_eg"
  echo
fi
echo "## الجديد"
echo
lines="$(jq -r '
  def section($k; $title):
    [.history[0] | select(.version == $v) | .changes[]? | select(.kind == $k) | select((.ar_eg // "") != "")] as $c
    | if ($c | length) > 0 then
        "### \($title)\n\n" + ($c | map("- " + .ar_eg) | join("\n")) + "\n"
      else empty end;
  [section("new"; "جديد"), section("improved"; "اتحسّن"), section("fixed"; "اتصلّح")] | join("\n")
' --arg v "$v" "$manifest")"
if [ -n "$lines" ]; then echo "$lines"; else echo "تصليحات صغيرة وتنضيف."; fi
echo
cat <<MD
## التثبيت

| الجهاز | الملف | أول مرة بس |
|---|---|---|
| أندرويد (S24 Ultra و Tab S11) | \`Kabar-$v-android.apk\` | افتحه على الجهاز واسمح بالتثبيت من المتصفح. بيتثبّت فوق أي نسخة قديمة من Kabar ومش بيمسح بياناتك. |
| ماك (Apple silicon) | \`Kabar-$v-mac.zip\` | فك الضغط، انقل Kabar.app لـ ~/Applications، وشغّل مرة واحدة \`xattr -dr com.apple.quarantine ~/Applications/Kabar.app\` (التوقيع ذاتي). |

الأجهزة المتثبّت عليها Kabar بتتحدّث لوحدها من قناة $channel_ar، والملفات دي محتاجها بس لجهاز جديد.

ملف \`SHA256SUMS.txt\` فيه بصمة SHA-256 لكل ملف (\`shasum -a 256 -c SHA256SUMS.txt\`).
MD

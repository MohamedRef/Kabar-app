#!/usr/bin/env bash
# Download the openly licensed corpora and dictionaries the content pipeline
# reads into data/raw/ (git-ignored). Every file is pinned by SHA-256 so a
# rebuild is reproducible; licenses are listed in ATTRIBUTION.md.
#
# Usage: scripts/fetch-sources.sh [freq|nusa|kaikki|jvwiki|all]   (default: freq)
set -euo pipefail

RAW="${KABAR_RAW:-data/raw}"
mkdir -p "$RAW"

fetch() { # url dest sha256 (empty sha = not pinned, e.g. rolling dumps)
  local url="$1" dest="$RAW/$2" sha="$3"
  if [[ -f "$dest" ]] && { [[ -z "$sha" ]] || echo "$sha  $dest" | shasum -a 256 -c --status; }; then
    echo "ok      $2"; return
  fi
  echo "fetch   $2"
  curl -fL --retry 4 --retry-delay 2 -o "$dest.part" "$url"
  if [[ -n "$sha" ]] && ! echo "$sha  $dest.part" | shasum -a 256 -c --status; then
    echo "checksum mismatch for $2 (upstream changed?)" >&2; rm -f "$dest.part"; exit 1
  fi
  mv "$dest.part" "$dest"
}

freq() {
  # wordfreq 3.1.1 (Apache-2.0 code, CC BY-SA 4.0 data): Indonesian blended
  # from Wikipedia, OpenSubtitles 2018 and Twitter.
  fetch "https://files.pythonhosted.org/packages/24/61/62835c475d69872d30689f284497853fe33fe1d6dd18f57346d13305861d/wordfreq-3.1.1-py3-none-any.whl" \
    wordfreq-3.1.1-py3-none-any.whl 4b1c6ecffc6198be3396d5cf871c4423ca71c907c231348d352dd54d62b97473
  unzip -o -q -j "$RAW/wordfreq-3.1.1-py3-none-any.whl" wordfreq/data/small_id.msgpack.gz -d "$RAW"
  # hermitdave/FrequencyWords (CC BY-SA 4.0): OpenSubtitles 2018, Indonesian.
  fetch "https://raw.githubusercontent.com/hermitdave/FrequencyWords/525f9b560de45753a5ea01069454e72e9aa541c6/content/2018/id/id_full.txt" \
    hermitdave-id_full.txt bd17bf66be3be86d4f07c5466582e6c99f43b91d365e771b98a3bad19f405e81
}

kaikki() {
  # Wiktionary extracts by kaikki.org (wiktextract), CC BY-SA 4.0 + GFDL.
  # Rolling files: not pinned; the import records the date it ran.
  fetch "https://kaikki.org/dictionary/Indonesian/kaikki.org-dictionary-Indonesian.jsonl" kaikki-id.jsonl ""
  fetch "https://kaikki.org/dictionary/Javanese/kaikki.org-dictionary-Javanese.jsonl" kaikki-jv.jsonl ""
}

nusa() {
  # Javanese text written by native speakers, for the Javanese frequency list.
  # NusaWrites (IndoNLP, Apache-2.0 repo) and NusaX (IndoNLP, CC BY-SA 4.0), pinned to commits.
  mkdir -p "$RAW/nusa"
  local nw="https://raw.githubusercontent.com/IndoNLP/nusa-writes/b337f511185cd44c9c57ff906ecef1af6165adbc/data"
  local nx="https://raw.githubusercontent.com/IndoNLP/nusax/75b0be5d982348332509f296e10d9bf9cb66d60f/datasets/sentiment/javanese"
  for split in train valid test; do
    fetch "$nw/nusa_alinea-paragraph-jav-$split.csv" "nusa/alinea-paragraph-$split.csv" ""
    fetch "$nw/nusa_kalimat-mt-jav-$split.csv" "nusa/kalimat-mt-$split.csv" ""
    fetch "$nx/$split.csv" "nusa/nusax-senti-$split.csv" ""
  done
}

jvwiki() {
  # Javanese Wikipedia articles (CC BY-SA 4.0) for the Javanese frequency list.
  fetch "https://dumps.wikimedia.org/jvwiki/latest/jvwiki-latest-pages-articles.xml.bz2" jvwiki-pages-articles.xml.bz2 ""
}

case "${1:-freq}" in
  freq) freq ;;
  kaikki) kaikki ;;
  nusa) nusa ;;
  jvwiki) jvwiki ;;
  all) freq; nusa; kaikki; jvwiki ;;
  *) echo "usage: $0 [freq|nusa|kaikki|jvwiki|all]" >&2; exit 2 ;;
esac

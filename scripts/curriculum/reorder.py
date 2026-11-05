# python3 reorder.py <level-or-unit-prefix>...   e.g. reorder.py bipa1 bipa2   (run from repo root)
# Keeps every unit in learn… → practice… → review order (PEDAGOGY §3.1, #224): any learn
# lesson listed after the unit's first practice lesson moves up to just before it. Lesson
# ids are renumbered to stay sequential; every reference in tracked files is rewritten.
import re, sys, os, subprocess
def reorder(prefixes):
    rename = {}
    pat = re.compile(r"^( +- \{ id: )(\S+)\.l(\d\d)(, kind: (\w+),.*)$")
    for path, lang in [("curriculum/indonesian.yaml", "id"), ("curriculum/javanese.yaml", "jv")]:
        lines = open(path).read().split("\n"); out = []; i = 0
        while i < len(lines):
            m = pat.match(lines[i])
            if not m or not any(m.group(2) == p or m.group(2).startswith(p + ".") for p in prefixes):
                out.append(lines[i]); i += 1; continue
            unit = m.group(2); block = []
            while i < len(lines) and (mm := pat.match(lines[i])) and mm.group(2) == unit:
                block.append(mm); i += 1
            kinds = [b.group(5) for b in block]
            if "practice" in kinds:
                fp = kinds.index("practice")
                block = block[:fp] + [b for b in block[fp:] if b.group(5) == "learn"] + [b for b in block[fp:] if b.group(5) != "learn"]
            for n, b in enumerate(block, 1):
                old, new = f"{unit}.l{b.group(3)}", f"{unit}.l{n:02d}"
                if old != new: rename[old] = (new, lang)
                out.append(f"{b.group(1)}{new}{b.group(4)}")
        open(path, "w").write("\n".join(out))
    # files: two-phase rename through temporary names
    for old, (new, lang) in rename.items():
        os.rename(f"content/lessons/{lang}/{old}.yaml", f"content/lessons/{lang}/{old}.yaml.tmp")
    for old, (new, lang) in rename.items():
        os.rename(f"content/lessons/{lang}/{old}.yaml.tmp", f"content/lessons/{lang}/{new}.yaml")
    # references in every tracked text file (curriculum already done), single regex pass
    if rename:
        rx = re.compile(r"\b(" + "|".join(re.escape(o) for o in sorted(rename, key=len, reverse=True)) + r")(?!\d)")
        files = subprocess.run(["git", "ls-files", "-co", "--exclude-standard"], capture_output=True, text=True).stdout.split()
        for f in files:
            if f.startswith("curriculum/") or not os.path.isfile(f): continue
            try: t = open(f, encoding="utf-8").read()
            except (UnicodeDecodeError, IsADirectoryError): continue
            t2 = rx.sub(lambda m: rename[m.group(1)][0], t)
            if t2 != t: open(f, "w", encoding="utf-8").write(t2); print("refs", f)
    print(len(rename), "lesson ids renumbered")
    return rename


if __name__ == "__main__":
    reorder(sys.argv[1:])

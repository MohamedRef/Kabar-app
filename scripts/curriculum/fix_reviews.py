# python3 fix_reviews.py <level-or-unit-prefix>...   (run from repo root)
# #224: make each unit's review lesson bring back at least one new word from every learn
# lesson. For units where some learn lessons are missing, one low-value review exercise
# (multiple choice, then dictation, speaking, translate) is replaced by a match_pairs
# exercise built from those lessons' own new_words glosses. Removal never drops another
# learn lesson's coverage or the exercise-type mix below 4. Check the diff and run fixmin.
import re, sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import yaml, ws

def forms(w):
    p = re.sub(r"\(.*?\)", "", w).lower().strip().rstrip("?"); p = " ".join(p.split())
    return {p} | {x.strip() for x in re.split(r"[/,]", p) if x.strip()}

SKIP = {"ar", "ar_eg", "en", "explain", "question_meaning", "meaning", "source", "answers_meaning", "id", "type", "words"}
def texts(o):
    if isinstance(o, dict):
        for k, v in o.items():
            if k not in SKIP: yield from texts(v)
    elif isinstance(o, list):
        for v in o: yield from texts(v)
    elif isinstance(o, str): yield o.lower()

def covers(ex, fs):
    tags = {str(t).lower() for t in ex.get("words") or []}
    blob = " ".join(texts(ex))
    return any(f in tags or re.search(r"(?<![\w-])" + re.escape(f) + r"(?![\w-])", blob) for f in fs)

PREF = ["multiple_choice", "dictation", "speaking", "translate", "listen"]
SECS = {"multiple_choice": 15, "listen": 20, "fill_blank": 20, "speaking": 20, "build_sentence": 30,
        "match_pairs": 35, "dictation": 35, "translate": 40}
def seconds(L):  # crates/core/src/lesson/pacing.rs
    t = sum(30 + 10 * len(s.get("examples") or []) for s in L.get("slides") or [])
    t += sum(SECS[e["type"]] for e in L["exercises"])
    t += 8 * len(((L.get("dialogue") or {}).get("lines")) or [])
    return t + 10 + 3 * len(L.get("new_words") or [])
for path, lang in [("curriculum/indonesian.yaml", "id"), ("curriculum/javanese.yaml", "jv")]:
    c = yaml.safe_load(open(path))
    for lv in c["levels"]:
        for u in lv["units"]:
            if not any(u["id"] == p or u["id"].startswith(p + ".") or lv["id"] == p for p in sys.argv[1:]): continue
            revs = [l for l in u["lessons"] if l["kind"] == "review"]
            if not revs: continue
            rpath = f"content/lessons/{lang}/{revs[-1]['id']}.yaml"
            if not os.path.exists(rpath): continue
            learn = []
            for l in u["lessons"]:
                p = f"content/lessons/{lang}/{l['id']}.yaml"
                if l["kind"] == "learn" and os.path.exists(p):
                    nw = yaml.safe_load(open(p)).get("new_words") or []
                    if nw: learn.append((l["id"], nw))
            R = yaml.safe_load(open(rpath)); exs = R["exercises"]
            cov = lambda e_list, nw: any(covers(e, forms(w["word"])) for e in e_list for w in nw)
            missing = [(lid, nw) for lid, nw in learn if not cov(exs, nw)]
            if not missing: continue
            groups = [missing[i:i + 6] for i in range(0, len(missing), 6)]
            text = open(rpath).read()
            for g in groups:
                per = 1 if len(g) >= 3 else (2 if len(g) == 2 else 3)
                pairs = []
                for lid, nw in g:
                    plain = [w for w in nw if " " not in w["word"] and not w["word"].startswith("-")] or nw
                    pairs += plain[:per]
                doc = yaml.safe_load(text); exs = doc["exercises"]; base = seconds(doc)
                kinds = {e["type"] for e in exs}
                victim = None
                for kind in PREF:
                    for i, e in enumerate(exs):
                        if e["type"] != kind: continue
                        rest = exs[:i] + exs[i + 1:]
                        fits = base - SECS[kind] + SECS["match_pairs"] <= 420
                        if fits and all(cov(rest, nw) for lid, nw in learn if cov(exs, nw)) and len({x["type"] for x in rest} | {"match_pairs"}) >= 4:
                            victim = i; break
                    if victim is not None: break
                if victim is None:
                    print("SKIP", u["id"], "no exercise can be replaced; edit the review by hand"); continue
                eid = exs[victim]["id"]
                block = f"  - id: {eid}\n    type: match_pairs\n    pairs:\n"
                for w in pairs:
                    word = re.sub(r"\s*\(.*?\)", "", w["word"]).strip()
                    block += f"      - text: {ws.Q(word)}\n        ar_eg: {ws.Q(w['ar_eg'])}\n        ar: {ws.Q(w['ar'])}\n        en: {ws.Q(w['en'])}\n"
                block += f"    words: {ws.flow([re.sub(r'\s*\(.*?\)', '', w['word']).strip() for w in pairs])}\n"
                m = re.search(rf"^  - id: {eid}\n(.*?)(?=^  - id: e|^\S)", text, re.M | re.S)
                text = text[:m.start()] + block + text[m.end():]
                print(u["id"], "review", revs[-1]["id"], eid, "->", ", ".join(w["word"] for w in pairs))
            yaml.safe_load(text); open(rpath, "w").write(text)

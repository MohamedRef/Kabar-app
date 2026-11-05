"""Lesson renderer. Exercise tuples (meanings are (ar_eg, ar, en) triples):
  ("match", [(target, eg, ar, en), ...], words?)
  ("mc", question_or_"", question_meaning3|None, options, answer_index, explain3, words)
  ("listen", text, options(ar_eg), answer_index, meaning3, words, question3)
  ("fill", sentence_with____, meaning3, options, answer, words)
  ("build", source3, answer, distractors, words)
  ("tr_from", target_text, {"ar_eg": [...], "ar": [...], "en": [...]}, words)
  ("tr_to", source3, answers, words)
  ("dict" | "speak", text, meaning3, words)
Lesson dict: title, can_do(eg, ar, en), slides=[{title, body(eg, ar, en), examples=[(t, eg, ar, en)]}],
ex=[...], dialogue=[(speaker, t, eg, ar, en)], words=[(word, pos, eg, ar, en[, arabic_script])].
"""
"""Word-set lesson authoring: compact specs -> lesson YAML in the repo's style."""
import re, yaml, os
SPECIAL = re.compile(r"(: |^[\[\]{}'\"&*!|>%@`#,?\-]|^- | #|:$|^\s|\s$|^(yes|no|true|false|null|on|off|~)$|^[0-9.+-]+$)", re.I)
def Q(x):
    x = str(x)
    return "'" + x.replace("'", "''") + "'" if SPECIAL.search(x) else x
def QF(x):  # flow-list item
    x = str(x)
    return "'" + x.replace("'", "''") + "'" if (SPECIAL.search(x) or re.search(r"[,\[\]{}#:'\"?]", x)) else x
def flow(xs): return "[" + ", ".join(QF(x) for x in xs) + "]"
def tri(ind, t):  # t = (eg, ar, en)
    return f"{ind}ar_eg: {Q(t[0])}\n{ind}ar: {Q(t[1])}\n{ind}en: {Q(t[2])}\n"
def block(ind, t):
    out = ""
    for k, v in zip(("ar_eg", "ar", "en"), t):
        out += f"{ind}{k}: |\n" + "".join(f"{ind}  {line}\n" for line in v.strip("\n").split("\n"))
    return out
def ex(i, e):
    k = e[0]; s = f"  - id: e{i:02d}\n"
    if k == "match":
        _, pairs, *w = e
        s += "    type: match_pairs\n    pairs:\n"
        for t, eg, ar, en in pairs:
            s += f"      - text: {Q(t)}\n" + tri("        ", (eg, ar, en))
        s += f"    words: {flow(w[0] if w else [p[0] for p in pairs])}\n"
    elif k == "mc":
        _, q, qm, opts, ans, expl, w = e
        s += "    type: multiple_choice\n"
        if q: s += f"    question: {Q(q)}\n"
        if qm: s += "    question_meaning:\n" + tri("      ", qm)
        s += f"    options: {flow(opts)}\n    answer: {ans}\n"
        if expl: s += "    explain:\n" + tri("      ", expl)
        s += f"    words: {flow(w)}\n"
    elif k == "listen":
        _, text, opts, ans, mean, w, *qq = e
        s += f"    type: listen\n    text: {Q(text)}\n"
        if qq and qq[0]: s += "    question:\n" + tri("      ", qq[0])
        s += f"    options: {flow(opts)}\n    answer: {ans}\n    meaning:\n" + tri("      ", mean) + f"    words: {flow(w)}\n"
    elif k == "fill":
        _, sent, mean, opts, ans, w = e
        s += f"    type: fill_blank\n    sentence: {Q(sent)}\n    meaning:\n" + tri("      ", mean)
        s += f"    options: {flow(opts)}\n    answers: {flow([ans] if isinstance(ans, str) else ans)}\n    words: {flow(w)}\n"
    elif k == "build":
        _, src, ans, dis, w = e
        s += "    type: build_sentence\n    source:\n" + tri("      ", src) + f"    answer: {Q(ans)}\n    distractors: {flow(dis)}\n    words: {flow(w)}\n"
    elif k == "tr_from":
        _, st, am, w = e
        s += f"    type: translate\n    direction: from_target\n    source_text: {Q(st)}\n    answers_meaning:\n"
        for key in ("ar_eg", "ar", "en"): s += f"      {key}: {flow(am[key])}\n"
        s += f"    words: {flow(w)}\n"
    elif k == "tr_to":
        _, sm, answers, w = e
        s += "    type: translate\n    direction: to_target\n    source_meaning:\n" + tri("      ", sm) + "    answers:\n" + "".join(f"      - {Q(a)}\n" for a in answers) + f"    words: {flow(w)}\n"
    elif k in ("dict", "speak"):
        _, text, mean, w = e
        s += f"    type: {'dictation' if k == 'dict' else 'speaking'}\n    text: {Q(text)}\n    meaning:\n" + tri("      ", mean) + f"    words: {flow(w)}\n"
    else: raise ValueError(k)
    return s
def lesson(L):
    s = f"schema: kabar.lesson/v1\nid: {L['id']}\nlanguage: {L.get('lang','id')}\ntitle: {Q(L['title'])}\ncan_do:\n" + tri("  ", L["can_do"])
    if L.get("slides"):
        s += "slides:\n"
        for sl in L["slides"]:
            s += f"  - title: {Q(sl['title'])}\n    body:\n" + block("      ", sl["body"])
            if sl.get("grammar"): s += f"    grammar: {sl['grammar']}\n"
            if sl.get("examples"):
                s += "    examples:\n"
                for t, eg, ar, en in sl["examples"]:
                    s += f"      - text: {Q(t)}\n" + tri("        ", (eg, ar, en))
    s += "exercises:\n" + "".join(ex(i + 1, e) for i, e in enumerate(L["ex"]))
    s += "dialogue:\n  lines:\n"
    for sp, t, eg, ar, en in L["dialogue"]:
        s += f"    - speaker: {Q(sp)}\n      text: {Q(t)}\n" + tri("      ", (eg, ar, en))
    s += "new_words:\n"
    for w in L["words"]:
        word, pos, eg, ar, en, *arabic = w
        s += f"  - word: {Q(word)}\n    pos: {pos}\n"
        if arabic and arabic[0]: s += f"    arabic: {arabic[0]}\n"
        s += tri("    ", (eg, ar, en))
    s += "review:\n  status: draft\n  verified: false\n"
    yaml.safe_load(s)
    return s

def esc(x): return x.replace("\\", "\\\\").replace('"', '\\"')

def add_lesson(root, unit, L, kind="learn", grammar=None):
    """Insert L as the lesson before the unit's review (renumbering the review)."""
    lesson(L)  # render first: fail before touching any file
    lang = L.get("lang", "id"); cur = {"id": "indonesian", "jv": "javanese"}[lang]
    cp = f"{root}/curriculum/{cur}.yaml"; c = open(cp).read()
    lines = re.findall(rf"^ +- \{{ id: {re.escape(unit)}\.l(\d\d), kind: (\w+),.*$", c, re.M)
    last_n, last_kind = int(lines[-1][0]), lines[-1][1]
    assert last_kind == "review", unit
    new_n = last_n; rev_new = last_n + 1
    old_rev = f"{unit}.l{last_n:02d}"; nr = f"{unit}.l{rev_new:02d}"; nid = f"{unit}.l{new_n:02d}"
    # rename the review lesson
    d = f"{root}/content/lessons/{lang}"
    os.rename(f"{d}/{old_rev}.yaml", f"{d}/{nr}.yaml")
    t = open(f"{d}/{nr}.yaml").read().replace(f"id: {old_rev}\n", f"id: {nr}\n", 1); open(f"{d}/{nr}.yaml", "w").write(t)
    m = re.search(rf"^( +)- \{{ id: {re.escape(old_rev)}, .*$", c, re.M)
    ind = m.group(1)
    g = f" grammar: [{', '.join(grammar)}]," if grammar else ""
    entry = f'{ind}- {{ id: {nid}, kind: {kind}, title: "{esc(L["title"])}", can_do: "{esc(L["can_do"][2])}",{g} minutes: 7 }}\n'
    c = c[:m.start()] + entry + c[m.start():].replace(f"id: {old_rev},", f"id: {nr},", 1)
    open(cp, "w").write(c)
    L = dict(L); L["id"] = nid
    open(f"{d}/{nid}.yaml", "w").write(lesson(L))
    return nid

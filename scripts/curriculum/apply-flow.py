# apply-flow.py spec.py... (repo root). For the inline-list bipa5-7 curriculum blocks, where apply.py fails (IndexError):
# new titles go after the unit's last learn lesson (before practice, #224); later ids shift.
import sys, os, re, importlib.util, subprocess
import os; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__))); import ws
CP = "curriculum/indonesian.yaml"; D = "content/lessons/id"
LINE = re.compile(r'^(?P<pre>.*?\{ id: )(?P<u>bipa\d\.u\d\d)\.l(?P<n>\d\d)(?P<rest>, kind: (?P<k>\w+), title: "(?P<t>(?:[^"\\]|\\.)*)".*)$')
def unit_lines(lines, unit):
    return [(i, m) for i, l in enumerate(lines) if (m := LINE.match(l)) and m["u"] == unit]
for f in sys.argv[1:]:
    sp = importlib.util.spec_from_file_location("m", f); m = importlib.util.module_from_spec(sp); sp.loader.exec_module(m)
    for unit, L in m.LESSONS:
        L = dict(L, lang="id"); lines = open(CP).read().split("\n"); ul = unit_lines(lines, unit)
        hit = [mm for _, mm in ul if mm["t"] == ws.esc(L["title"])]
        if hit:
            lid = f"{unit}.l{hit[0]['n']}"; open(f"{D}/{lid}.yaml", "w").write(ws.lesson(dict(L, id=lid))); print("regen", lid); continue
        ws.lesson(dict(L, id="x"))
        kinds = [mm["k"] for _, mm in ul]; p = kinds.index("practice")  # 0-based index of first practice
        new_n = p + 1; nid = f"{unit}.l{new_n:02d}"
        ren = {}
        for i, mm in ul[p:]:
            old = int(mm["n"]); ren[f"{unit}.l{old:02d}"] = f"{unit}.l{old+1:02d}"
            lines[i] = f"{mm['pre']}{unit}.l{old+1:02d}{mm['rest']}"
        for old in ren: os.rename(f"{D}/{old}.yaml", f"{D}/{old}.yaml.tmp")
        for old, new in ren.items():
            t = open(f"{D}/{old}.yaml.tmp").read().replace(f"id: {old}\n", f"id: {new}\n", 1)
            open(f"{D}/{new}.yaml", "w").write(t); os.remove(f"{D}/{old}.yaml.tmp")
        i0, m0 = ul[p - 1]
        ind = re.match(r"^(\s*)", lines[ul[1][0]]).group(1)
        entry = f'{ind}{{ id: {nid}, kind: learn, title: "{ws.esc(L["title"])}", can_do: "{ws.esc(L["can_do"][2])}", minutes: 7 }},'
        lines.insert(i0 + 1, entry)
        open(CP, "w").write("\n".join(lines))
        open(f"{D}/{nid}.yaml", "w").write(ws.lesson(dict(L, id=nid))); print("add", nid)

# python3 apply.py <spec.py>...   (run from the repo root)
# Each spec defines LESSONS = [(unit_id, lesson_dict), ...]. A lesson whose title already
# exists in that unit is regenerated in place; otherwise it is inserted before the unit's first
# practice lesson (reorder.py renumbers the practice/review lessons and rewrites references).
# A lesson with replaces="Old title" rewrites that lesson in place under its new title.
import sys, os, re, importlib.util
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ws
from reorder import reorder
root = os.getcwd()
for f in sys.argv[1:]:
    spec = importlib.util.spec_from_file_location("m", f); m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
    for unit, L in m.LESSONS:
        lang = "jv" if unit.startswith("j") else "id"; L = dict(L, lang=lang)
        cur = open(f"{root}/curriculum/{'javanese' if lang == 'jv' else 'indonesian'}.yaml").read()
        find = lambda t: re.search(rf'id: ({re.escape(unit)}\.l\d\d), kind: (\w+), title: "{re.escape(ws.esc(t))}"', cur)
        old = L.get("replaces") and find(L["replaces"])
        hit = old or find(L["title"])
        if hit and hit.group(2) != "learn":
            # never overwrite a practice or review lesson that happens to share the title
            sys.exit(f"{unit}: '{L['title']}' is the title of {hit.group(1)} ({hit.group(2)}); give the word lesson another title")
        if old:
            # a rewrite under a new title: same id, new curriculum title and can_do
            cpath = f"{root}/curriculum/{'javanese' if lang == 'jv' else 'indonesian'}.yaml"
            line = re.search(rf'^.*id: {re.escape(hit.group(1))},.*$', cur, re.M).group(0)
            fixed = re.sub(r'title: "(?:[^"\\]|\\.)*", can_do: "(?:[^"\\]|\\.)*"',
                           lambda _: f'title: "{ws.esc(L["title"])}", can_do: "{ws.esc(L["can_do"][2])}"', line)
            open(cpath, "w").write(cur.replace(line, fixed))
            L = dict(L, id=hit.group(1)); open(f"{root}/content/lessons/{lang}/{hit.group(1)}.yaml", "w").write(ws.lesson(L)); print("rewrite", hit.group(1))
        elif hit:
            L = dict(L, id=hit.group(1)); open(f"{root}/content/lessons/{lang}/{hit.group(1)}.yaml", "w").write(ws.lesson(L)); print("regen", hit.group(1))
        else:
            L.setdefault("id", "x"); nid = ws.add_lesson(root, unit, L)
            # #224: word lessons go before the unit's practice lesson, ids stay sequential
            moved = reorder([unit]); print("add", moved.get(nid, (nid,))[0])

# unk.py lid... : words in a lesson's target text not taught at or before it
import yaml,re,sys,os
LANG="jv" if sys.argv[1].startswith("j") else "id"
cur=yaml.safe_load(open("curriculum/"+("javanese" if LANG=="jv" else "indonesian")+".yaml"))
order=[l["id"] for lv in cur["levels"] for u in lv["units"] for l in u["lessons"]]
def forms(w):
    p=re.sub(r"\(.*?\)","",w).lower().strip().rstrip("?")
    s={p}|{x.strip() for x in re.split(r"[/,]",p) if x.strip()}
    return s
taught={}
for i,l in enumerate(order):
    for w in yaml.safe_load(open(f"content/lessons/{LANG}/{l}.yaml")).get("new_words") or []:
        for f in forms(w["word"]): taught.setdefault(f,i)
SKIP={"ar","ar_eg","en","speaker","id","type","pos","arabic","explain","question_meaning","meaning","source","source_meaning","answers_meaning","can_do","body","title","review","new_words","question"}
def texts(o):
    if isinstance(o,dict):
        for k,v in o.items():
            if k in SKIP: continue
            yield from texts(v)
    elif isinstance(o,list):
        for v in o: yield from texts(v)
    elif isinstance(o,str): yield o
for lid in sys.argv[1:]:
    i=order.index(lid); d=yaml.safe_load(open(f"content/lessons/{LANG}/{lid}.yaml"))
    bad=set()
    for part in ("slides","exercises","dialogue"):
        for t in texts(d.get(part)):
            for tok in re.findall(r"[A-Za-z][\w-]*",t):
                if tok[0].isupper() and tok.lower() not in taught: continue
                w=tok.lower(); base=re.sub(r"(nya|ku|mu)$","",w)
                if taught.get(w,9999)<=i or taught.get(base,9999)<=i: continue
                bad.add(f"{w}({order[taught[w]] if w in taught else '-'})")
    print(lid, " ".join(sorted(bad)))

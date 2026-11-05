# chk.py lang word... : for each candidate print taught-where or freq rank
import yaml,re,sys
lang=sys.argv[1]; cur={"id":"indonesian","jv":"javanese"}[lang]
def forms(w):
    p=re.sub(r"\(.*?\)","",w).lower().strip().rstrip("?")
    return {p}|{x.strip() for x in re.split(r"[/,]",p) if x.strip()}
c=yaml.safe_load(open(f"curriculum/{cur}.yaml")); taught={}
for lv in c["levels"]:
    for u in lv["units"]:
        for l in u["lessons"]:
            d=yaml.safe_load(open(f"content/lessons/{lang}/{l['id']}.yaml"))
            for w in d.get("new_words") or []:
                for f in forms(w["word"]): taught.setdefault(f,l["id"])
rank={}
for l in open(f"content/freq/{lang}.tsv"):
    if l.startswith("#"): continue
    r=l.split("\t"); rank.setdefault(r[1],int(r[0]))
words=" ".join(sys.argv[2:]).split(",")
new=[]
for w in [x.strip() for x in words if x.strip()]:
    if w in taught: print(f"  TAUGHT {w} {taught[w]}")
    else: new.append((rank.get(w,99999),w))
print("NEW:",", ".join(f"{w}({r})" for r,w in sorted(new)))

# gap.py <lang> <topN>: frequency words (rank<=N) not taught anywhere, with whether any lesson uses them
import yaml,re,sys
lang,N=sys.argv[1],int(sys.argv[2])
cur={"id":"indonesian","jv":"javanese"}[lang]
def forms(w):
    p=re.sub(r"\(.*?\)","",w).lower().strip().rstrip("?")
    return {p}|{x.strip() for x in re.split(r"[/,]",p) if x.strip()}|set(p.split())
c=yaml.safe_load(open(f"curriculum/{cur}.yaml"))
taught=set()
for lv in c["levels"]:
    for u in lv["units"]:
        for l in u["lessons"]:
            d=yaml.safe_load(open(f"content/lessons/{lang}/{l['id']}.yaml"))
            for w in d.get("new_words") or []: taught|=forms(w["word"])
ex=set()
for f in [f"content/freq/{lang}.core-exclude.tsv"]:
    for l in open(f):
        if not l.startswith("#") and l.strip(): ex.add(l.split("\t")[0])
rows=[l.rstrip("\n").split("\t") for l in open(f"content/freq/{lang}.tsv") if not l.startswith("#")]
out=[r for r in rows if int(r[0])<=N and r[1] not in taught and r[1] not in ex]
print(len(out),"untaught in top",N)
print(" ".join(f"{r[1]}" for r in out))

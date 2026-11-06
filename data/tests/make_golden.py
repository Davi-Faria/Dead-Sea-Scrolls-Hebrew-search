#!/usr/bin/env python3
"""Make tests/golden.json: answers of the ORIGINAL Python/Polars version, for the Rust tests to match.

You only need this if you change what the analyses compute. It needs the old Python version
(dss_stats.py, build_data.py and data/dss.json from the pre-Rust repository) and Polars:

    pip install polars
    python tests/make_golden.py --python-repo ../dss-hebrew-search-python

Polars does not always order rows that tie completely (for example the same word as Hebrew and
as Aramaic with the same count) the same way twice, so every question is asked three times and
only those with identical answers are kept. Rows beyond the first 40 are dropped to keep the file
small (the test then compares just those rows, and the whole summary).
"""
import argparse, json, pathlib, sys

parser = argparse.ArgumentParser()
parser.add_argument("--python-repo", required=True, help="folder with dss_stats.py and data/dss.json")
parser.add_argument("--out", default=str(pathlib.Path(__file__).with_name("golden.json")))
parser.add_argument("--runs", type=int, default=3, help="how many separate processes to ask")
parser.add_argument("--single", action="store_true", help="(internal) answer everything once and print it")
args = parser.parse_args()
repo = pathlib.Path(args.python_repo).resolve()
sys.path.insert(0, str(repo))
from dss_stats import Corpus, query

c = Corpus.load(repo / "data" / "dss.json")
KEEP_ROWS = 40

cases = []
def add(analysis, **params):
    cases.append((analysis, {k: str(v) for k, v in params.items()}))

# ---- frequencies
add("freq")
add("freq", top=0)
add("freq", unit="form")
add("freq", unit="form", top=0)
add("freq", corpus="bib", top=60)
add("freq", corpus="all", top=60)
add("freq", corpus="all", unit="form", top=60)
add("freq", lang="a", top=40)
add("freq", lang="h", top=40, unit="form")
add("freq", rec="skipfull", top=50)
add("freq", rec="skipany", top=50)
add("freq", rec="skipany", unit="form", top=50)
add("freq", scrolls="1QS, CD", top=50)
add("freq", scrolls="4Q4*", top=50)
add("freq", scrolls="1QS", corpus="all", top=0)
add("freq", **{"min": 50}, top=0)
add("freq", **{"min": 1000})
add("freq", exclude="ם א ל", top=30)
add("freq", exclude="כל,אשר;את", top=30, unit="form")
add("freq", scrolls="nosuchscroll")
add("freq", scrolls="11Q*", corpus="all", top=40)

# ---- distribution
words = ["יחד", "עין", "עינ", "אל", "ארץ", "אור", "ברית", "תורה", "מלך", "מלכ", "עור", "דבר", "שלום", "ל", "ם", "אלהימ"]
for w in words:
    add("dist", word=w)
for w in ["יחד", "עין", "אל", "ברית"]:
    add("dist", word=w, by="cave")
    add("dist", word=w, sort="order")
    add("dist", word=w, by="cave", sort="order")
    add("dist", word=w, unit="form")
    add("dist", word=w, unit="form", by="cave", sort="order")
    add("dist", word=w, corpus="bib")
    add("dist", word=w, corpus="all", by="cave")
    add("dist", word=w, lang="a")
    add("dist", word=w, rec="skipany", by="cave")
    add("dist", word=w, rec="skipfull")
    add("dist", word=w, scrolls="1QS, CD, 4Q26*", sort="order")
    add("dist", word=w, top=5)
add("dist", word="xyz")
add("dist", word="יחדד")        # not a word -> suggestions
add("dist", word="יח")          # prefix -> suggestions
add("dist", word="עי", unit="form")
add("dist", word="שמע", corpus="all", sort="order")
add("dist", word="מקדש", scrolls="11Q19")
add("dist", word="יחד", scrolls="nosuch")

# ---- collocations
for w in ["יחד", "עין", "ברית", "אל", "משפט", "שלום"]:
    add("colloc", word=w)
for w in ["יחד", "ברית"]:
    for window in (1, 2, 8, 30):
        add("colloc", word=w, window=window, top=30)
    for sort in ("ll", "logdice", "count"):
        add("colloc", word=w, sort=sort, top=30)
    add("colloc", word=w, unit="form", top=30)
    add("colloc", word=w, unit="form", sort="logdice", top=30)
    add("colloc", word=w, corpus="bib", top=30)
    add("colloc", word=w, corpus="all", top=30)
    add("colloc", word=w, lang="a", top=30)
    add("colloc", word=w, rec="skipfull", top=30)
    add("colloc", word=w, rec="skipany", top=30)
    add("colloc", word=w, scrolls="1QS, 1QSa, CD", top=30, **{"min": 1})
    add("colloc", word=w, exclude="ם א ל כל", top=30)
    add("colloc", word=w, top=0, **{"min": 2})
add("colloc", word="עין", window=4, top=0, **{"min": 1}, corpus="all")
add("colloc", word="דנ", lang="a", top=30, **{"min": 1})
add("colloc", word="xyz")
add("colloc", word="יחד", scrolls="nosuch")

# ---- keyness
add("keyness", target="1QS")
add("keyness", target="1QS", top=0)
add("keyness", target="1QS", direction="under")
add("keyness", target="1QS", direction="both", top=60)
add("keyness", target="1QS", sort="log_ratio")
add("keyness", target="1QS", sort="log_ratio", direction="under")
add("keyness", target="1QS", sort="log_ratio", direction="both", top=60)
add("keyness", target="1QS", reference="4Q25*")
add("keyness", target="4Q26*", reference="CD, 1QS")
add("keyness", target="CD", unit="form")
add("keyness", target="1QM", unit="form", direction="both", top=60)
add("keyness", target="1QpHab", corpus="all")
add("keyness", target="1QpHab", corpus="bib", **{"min": 1})
add("keyness", target="1QIsaa", corpus="bib", reference="4Q*")
add("keyness", target="1Qisaa", corpus="bib")
add("keyness", target="1QS", lang="h")
add("keyness", target="4Q2*", lang="a", top=40)
add("keyness", target="1QS", rec="skipfull")
add("keyness", target="1QS", rec="skipany", unit="form")
add("keyness", target="1QS", exclude="ם א ל כל")
add("keyness", target="1QS", scrolls="CD", top=20)   # scrolls setting is ignored
add("keyness", target="1QS, 1QSa, 1QSb", reference="CD")
add("keyness", target="11Q19", **{"min": 20}, top=0)
add("keyness", target="1QS, CD", reference="1QS", top=20)  # scroll in both -> target

# ---- errors
add("freq", corpus="nope")
add("freq", lang="x")
add("freq", rec="x")
add("freq", unit="x")
add("freq", top="abc")
add("freq", top="6000")
add("freq", top="-1")
add("freq", **{"min": "0"})
add("dist", word="")
add("dist", word="   ")
add("dist", word="abc")
add("dist", word="יחד אל")
add("dist", word="יחד", by="x")
add("dist", word="יחד", sort="x")
add("colloc", word="יחד", window=0)
add("colloc", word="יחד", window=31)
add("colloc", word="יחד", window="x")
add("colloc", word="יחד", sort="x")
add("colloc", word="")
add("keyness", target="")
add("keyness", target="  ")
add("keyness", target="zzz")
add("keyness", target="1QS", reference="zzz")
add("keyness", target="1QS", sort="x")
add("keyness", target="1QS", direction="x")

def ask(analysis, params):
    try:
        return {"analysis": analysis, "params": params, "ok": True, "result": query(c, analysis, params).to_dict()}
    except ValueError as e:
        return {"analysis": analysis, "params": params, "ok": False, "error": str(e)}

if args.single:
    print(json.dumps([ask(a, p) for a, p in cases], ensure_ascii=False))
    sys.exit(0)

# Polars orders tied rows differently from one process to the next, so each run is a new process.
import subprocess
runs = []
for n in range(args.runs):
    done = subprocess.run([sys.executable, __file__, "--python-repo", str(repo), "--single"], capture_output=True, text=True, check=True)
    runs.append(json.loads(done.stdout))

kept, dropped = [], 0
for answers in zip(*runs):
    if len({json.dumps(a, sort_keys=True, ensure_ascii=False) for a in answers}) != 1:
        dropped += 1
        print("not deterministic, left out:", answers[0]["analysis"], answers[0]["params"])
        continue
    entry = answers[0]
    if entry["ok"] and len(entry["result"]["rows"]) > KEEP_ROWS:
        entry["result"]["rows"] = entry["result"]["rows"][:KEEP_ROWS]
        entry["truncated"] = True
    kept.append(entry)
pathlib.Path(args.out).write_text(json.dumps(kept, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
print(f"{len(kept)} cases written to {args.out} ({dropped} left out)")

#!/usr/bin/env python3
"""Build data/dss.json for the search page from the ETCBC/CACCHT Dead Sea Scrolls dataset.

You only need this script if you want to rebuild the data (for a newer version of
the dataset, or after changing what is exported). The repository ships a prebuilt
data/dss.json.

Usage:
    pip install -r requirements-build.txt
    python build_data.py                       # downloads the needed Text-Fabric files, then builds
    python build_data.py --tf-dir ../dss/tf/2.0.1   # or use a local clone of github.com/ETCBC/dss

Output format: see "Data format" in README.md.
"""

import argparse
import json
import re
import sys
import unicodedata
import time
import urllib.request
from pathlib import Path

# The dataset commit that the shipped data/dss.json was built from.
# Change this (or pass --ref master) to build from a newer version.
DEFAULT_REF = "ac91a978d630f426e327f88eb0eeb85d6e67210a"
DEFAULT_VERSION = "2.0.1"
RAW_URL = "https://raw.githubusercontent.com/ETCBC/dss/{ref}/tf/{version}/{name}.tf"

# Text-Fabric needs these two to understand the corpus structure. (The dataset's own
# otext.tf points at many transliteration files we do not need, so we write a small one.)
CORE_FILES = ["otype", "oslots"]
OTEXT = """@config
@acronym=dss
@description=Minimal configuration written by build_data.py
@sectionFeatures=scroll,fragment,line
@sectionTypes=scroll,fragment,line
@fmt:text-orig-full={glyph}{after}

"""
# Features this script reads (see docs/feature_documentation.md in ETCBC/dss).
FEATURES = [
    "scroll", "fragment", "line",          # location: scroll, fragment/column, line
    "biblical", "book", "chapter", "verse",  # biblical material and its reference
    "glyph", "full",                       # the text of a word
    "glex", "sp",                          # lexeme (pointed Hebrew) and part of speech
    "lang", "after", "type",               # language, space-after, kind of sign
    "rec", "unc", "cor", "rem",            # editorial marks on signs
]

# Per-word status flags, written to the file as one base-32 character per word.
BIT_REC_PART = 1   # some letters reconstructed by a modern editor
BIT_REC_FULL = 2   # all letters reconstructed
BIT_UNC = 4        # some letters uncertain
BIT_COR = 8        # corrected (by an ancient or modern editor)
BIT_REM = 16       # removed (by an ancient or modern editor)
B32 = "0123456789abcdefghijklmnopqrstuv"

HEBREW_LETTER = re.compile("[\u05d0-\u05ea]")


# --------------------------------------------------------------------------
# Pure helpers (covered by tests/test_build.py)
# --------------------------------------------------------------------------

def hebrew_letters(text):
    """Keep only the Hebrew consonant letters (drops points, marks, spaces).

    Lexemes use presentation forms such as U+FB2A (shin with shin dot); decomposing
    first turns them back into the plain letter plus a point that is then dropped.
    """
    return "".join(HEBREW_LETTER.findall(unicodedata.normalize("NFD", text or "")))


def display_text(glyph, full, lang):
    """The text of one token as shown on the page."""
    raw = glyph or full or ""
    if lang == "g":                      # a handful of Greek tokens
        return raw.strip()
    letters = hebrew_letters(raw)
    if letters:
        return letters
    if "\u03b5" in raw or not raw.strip():   # ε marks a missing sign
        return "\u2026"
    return raw.strip()                   # punctuation such as the sof pasuq


def merge_tokens(parts):
    """Combine tokens written together (e.g. prefix + stem) into one orthographic word.

    Each part is a dict with keys: text, lemma, sp, lang, bits.
    Returns a dict with text, lemma (or None), lang ('h', 'a' or 'g') and bits.
    """
    text = "".join(p["text"] for p in parts)
    stem = next((p for p in parts if p["sp"] != "ptcl" and p["lemma"]), None)
    lang_source = stem or parts[0]
    lang = lang_source["lang"] if lang_source["lang"] in ("a", "g") else "h"

    bits = 0
    for p in parts:
        bits |= p["bits"] & (BIT_UNC | BIT_COR | BIT_REM)
    lettered = [p for p in parts if hebrew_letters(p["text"])]
    rec_flags = [p["bits"] & (BIT_REC_PART | BIT_REC_FULL) for p in lettered]
    if any(rec_flags):
        all_full = all(f & BIT_REC_FULL for f in rec_flags)
        bits |= BIT_REC_FULL if all_full else BIT_REC_PART

    return {"text": text, "lemma": stem["lemma"] if stem else None, "lang": lang, "bits": bits}


def group_tokens(tokens):
    """Group the tokens of one line into orthographic words.

    The dataset writes prefixes (ו ה ב כ ל מ ש) as separate tokens with no space
    after them. A token with a space after it ends the word.
    Each token is a dict with an extra key 'after' (None = no space follows).
    """
    words, current = [], []
    for token in tokens:
        current.append(token)
        if token["after"] is not None:
            words.append(merge_tokens(current))
            current = []
    if current:
        words.append(merge_tokens(current))
    return words


# --------------------------------------------------------------------------
# Download and build
# --------------------------------------------------------------------------

def download_features(cache_dir, ref, version):
    target = Path(cache_dir) / version
    target.mkdir(parents=True, exist_ok=True)
    (target / "otext.tf").write_text(OTEXT, encoding="utf-8")
    for name in CORE_FILES + FEATURES:
        path = target / f"{name}.tf"
        if path.exists() and path.stat().st_size > 0:
            continue
        url = RAW_URL.format(ref=ref, version=version, name=name)
        print(f"  downloading {name}.tf", flush=True)
        for attempt in range(3):
            try:
                with urllib.request.urlopen(url, timeout=60) as response:
                    path.write_bytes(response.read())
                break
            except Exception as error:   # network hiccup: retry, then give up
                if attempt == 2:
                    sys.exit(f"Could not download {url}: {error}")
                time.sleep(2)
    return target


def build(tf_dir, ref):
    try:
        from tf.fabric import Fabric
    except ImportError:
        sys.exit("Text-Fabric is not installed. Run: pip install -r requirements-build.txt")

    print("Loading Text-Fabric data ...", flush=True)
    # Text-Fabric mishandles relative paths that start with a dot, so resolve to an absolute path.
    api = Fabric(locations=str(Path(tf_dir).resolve()), silent="deep").load(" ".join(FEATURES), silent="deep")
    if not api:
        sys.exit(f"Text-Fabric could not load the features from {tf_dir}. "
                 f"Check that it contains {', '.join(FEATURES)} (.tf files).")
    F, L, E = api.F, api.L, api.E
    rec, unc, cor, rem = F.rec.data, F.unc.data, F.cor.data, F.rem.data

    def token(w):
        slots = E.oslots.s(w)
        letters = [s for s in slots if F.type.v(s) == "cons"]
        bits = 0
        n_rec = sum(1 for s in letters if s in rec)
        if n_rec:
            bits |= BIT_REC_FULL if n_rec == len(letters) else BIT_REC_PART
        if any(s in unc for s in slots):
            bits |= BIT_UNC
        if any(s in cor for s in slots):
            bits |= BIT_COR
        if any(s in rem for s in slots):
            bits |= BIT_REM
        glex = F.glex.v(w)
        return {
            "text": display_text(F.glyph.v(w), F.full.v(w), F.lang.v(w)),
            "lemma": hebrew_letters(glex) if glex else None,
            "sp": F.sp.v(w),
            "lang": F.lang.v(w),
            "bits": bits,
            "after": F.after.v(w),
        }

    scrolls, lemma_ids, lemma_list, lines = [], {}, [], []
    n_words = 0
    print("Converting scrolls ...", flush=True)
    for sc in F.otype.s("scroll"):
        scroll_id = F.scroll.v(sc)
        scroll_index = len(scrolls)
        scrolls.append(scroll_id)
        for frag in L.d(sc, "fragment"):
            frag_label = F.fragment.v(frag) or ""
            for ln in L.d(frag, "line"):
                tokens = [token(w) for w in L.d(ln, "word")]
                if not tokens:
                    continue
                words = group_tokens(tokens)
                lemmas = []
                for word in words:
                    if word["lemma"]:
                        key = (word["lemma"], word["lang"])
                        if key not in lemma_ids:
                            lemma_ids[key] = len(lemma_list)
                            lemma_list.append(key)
                        lemmas.append(lemma_ids[key])
                    else:
                        lemmas.append(-1)
                biblical = 1 if F.biblical.v(ln) else 0
                bref = ""
                if biblical:
                    first = next((w for w in L.d(ln, "word") if F.book.v(w)), None)
                    if first is not None:
                        bref = f"{F.book.v(first)} {F.chapter.v(first)}:{F.verse.v(first)}"
                lines.append([
                    scroll_index,
                    frag_label,
                    F.line.v(ln) or "",
                    [word["text"] for word in words],
                    lemmas,
                    "".join(B32[word["bits"]] for word in words),
                    biblical,
                    bref,
                ])
                n_words += len(words)

    return {
        "meta": {
            "source": "CACCHT Dead Sea Scrolls dataset (ETCBC/dss), based on data files of Martin G. Abegg",
            "url": "https://github.com/ETCBC/dss",
            "ref": ref,
            "license": "CC BY-NC 4.0",
            "words": n_words,
            "lines": len(lines),
        },
        "scrolls": scrolls,
        "lemmas": [lemma for lemma, _ in lemma_list],
        "lemmaLang": [lang for _, lang in lemma_list],
        "lines": lines,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--tf-dir", help="folder with the .tf files (e.g. a local ETCBC/dss clone: tf/2.0.1)")
    parser.add_argument("--ref", default=DEFAULT_REF, help="git commit/branch of ETCBC/dss to download")
    parser.add_argument("--version", default=DEFAULT_VERSION, help="dataset version folder inside tf/")
    parser.add_argument("--cache", default=".cache/dss-tf", help="where downloaded .tf files are kept")
    parser.add_argument("--out", default="data/dss.json", help="output file")
    args = parser.parse_args()

    tf_dir = Path(args.tf_dir) if args.tf_dir else download_features(args.cache, args.ref, args.version)
    data = build(tf_dir, args.ref if not args.tf_dir else "local")

    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open("w", encoding="utf-8") as handle:
        json.dump(data, handle, ensure_ascii=False, separators=(",", ":"))
    size_mb = out.stat().st_size / 1e6
    print(f"Wrote {out} ({size_mb:.1f} MB): {data['meta']['words']:,} words in "
          f"{data['meta']['lines']:,} lines, {len(data['scrolls'])} scrolls, {len(data['lemmas']):,} lexemes.")


if __name__ == "__main__":
    main()

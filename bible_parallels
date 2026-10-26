"""Data for the Parallels tab: which manuscripts contain each biblical chapter, and where
other texts (pesharim, the Melchizedek text, rewritten Torah and so on) quote it.

Used by build_data.py; it can also be run on its own on an existing data/dss.json plus the
verse segments (see build_data.py).

Two kinds of data are produced:

* "segments": every run of words in a biblical manuscript that belongs to one verse, as
  [line, book, chapter, verse, first word, end word]. The page uses them to line manuscripts
  up verse by verse.
* "quotes": stretches of non-biblical text that match a biblical verse for at least four
  words in a row. This is a heuristic (see find_quotes), not a scholarly identification.
"""

import collections
import unicodedata

BOOK_NAMES = {
    "Gen": "Genesis", "Ex": "Exodus", "Lev": "Leviticus", "Num": "Numbers", "Deut": "Deuteronomy",
    "Josh": "Joshua", "Judg": "Judges", "1Sam": "1 Samuel", "2Sam": "2 Samuel", "1Kgs": "1 Kings",
    "2Kgs": "2 Kings", "Is": "Isaiah", "Jer": "Jeremiah", "Ezek": "Ezekiel", "Hos": "Hosea",
    "Joel": "Joel", "Amos": "Amos", "Obad": "Obadiah", "Jonah": "Jonah", "Mic": "Micah",
    "Nah": "Nahum", "Hab": "Habakkuk", "Zeph": "Zephaniah", "Hag": "Haggai", "Zech": "Zechariah",
    "Mal": "Malachi", "Ps": "Psalms", "Job": "Job", "Prov": "Proverbs", "Ruth": "Ruth",
    "Song": "Song of Songs", "Eccl": "Ecclesiastes", "Lam": "Lamentations", "Esth": "Esther",
    "Dan": "Daniel", "Ezra": "Ezra", "Neh": "Nehemiah", "1Chr": "1 Chronicles", "2Chr": "2 Chronicles",
}
BOOK_ORDER = list(BOOK_NAMES)   # canonical order

_FINALS = str.maketrans("\u05da\u05dd\u05df\u05e3\u05e5", "\u05db\u05de\u05e0\u05e4\u05e6")


def skeleton(word):
    """A spelling-neutral form of a word, so that full and defective spellings match.

    Keeps only the Hebrew consonants, makes final letters ordinary, and drops ו and י, which
    the scrolls often add or leave out as vowel letters.
    """
    letters = "".join(c for c in unicodedata.normalize("NFD", word or "") if "\u05d0" <= c <= "\u05ea")
    return letters.translate(_FINALS).replace("\u05d5", "").replace("\u05d9", "")


def verse_sequences(lines, segments):
    """{(book, chapter, verse): {scroll index: [skeleton words in reading order]}}"""
    out = collections.defaultdict(lambda: collections.defaultdict(list))
    for li, book, chapter, verse, start, end in segments:
        scroll = lines[li][0]
        out[(book, chapter, verse)][scroll].extend(skeleton(w) for w in lines[li][3][start:end])
    return out


def ngram_index(sequences, n=4, min_chars=9):
    """Map every run of n consecutive words (at least min_chars letters in all) to the verses it occurs in."""
    index = collections.defaultdict(set)
    for ref, by_scroll in sequences.items():
        for words in by_scroll.values():
            for i in range(len(words) - n + 1):
                gram = tuple(words[i:i + n])
                if all(gram) and sum(len(w) for w in gram) >= min_chars:
                    index[gram].add(ref)
    return index


def find_quotes(lines, segments, n=4, min_chars=9, max_verses=6, ambiguous_min_words=6):
    """Find stretches of non-biblical text that match biblical text for at least n words in a row.

    Returns [[line, first word, end word, book, chapter, first verse, last verse, span], ...].
    Pieces of one quotation that run over several lines (or fit several chapters) share a span number.
    A match that fits more than max_verses different verses is thrown away as a formula
    (for example "the LORD your God") that tells us nothing about which verse is meant, and a
    match that fits several verses must be at least ambiguous_min_words long.
    """
    index = ngram_index(verse_sequences(lines, segments), n, min_chars)
    quotes = []
    span_number = 0
    i = 0
    while i < len(lines):
        # one fragment at a time, so that a quotation can continue onto the next line
        j = i
        while j + 1 < len(lines) and lines[j + 1][0] == lines[i][0] and lines[j + 1][1] == lines[i][1]:
            j += 1
        if not lines[i][6]:
            flat = []
            for li in range(i, j + 1):
                if lines[li][6]:
                    continue
                for pos, word in enumerate(lines[li][3]):
                    flat.append((skeleton(word), li, pos))
            spans = []     # [first, end, votes]
            for k in range(len(flat) - n + 1):
                gram = tuple(item[0] for item in flat[k:k + n])
                if not all(gram) or gram not in index:
                    continue
                if spans and k <= spans[-1][1]:
                    spans[-1][1] = k + n
                    spans[-1][2].update(index[gram])
                else:
                    spans.append([k, k + n, collections.Counter(index[gram])])
            for first, end, votes in spans:
                top = max(votes.values())
                chosen = sorted(ref for ref, count in votes.items() if count >= top * 0.5)
                if len(chosen) > max_verses or (len(chosen) > 1 and end - first < ambiguous_min_words):
                    continue
                span_number += 1
                # per line, per chapter
                runs = []
                for k in range(first, end):
                    _, li, pos = flat[k]
                    if runs and runs[-1][0] == li and runs[-1][2] == pos:
                        runs[-1][2] = pos + 1
                    else:
                        runs.append([li, pos, pos + 1])
                chapters = collections.defaultdict(list)
                for book, chapter, verse in chosen:
                    chapters[(book, chapter)].append(verse)
                for li, s, e in runs:
                    for (book, chapter), verses in chapters.items():
                        quotes.append([li, s, e, book, chapter, min(verses), max(verses), span_number])
        i = j + 1
    return quotes


def build_bible(data, segments):
    """The contents of data/bible.json."""
    lines = data["lines"]
    quotes = find_quotes(lines, segments)
    present = {s[1] for s in segments} | {q[3] for q in quotes}
    books = [b for b in BOOK_ORDER if b in present]
    index = {b: i for i, b in enumerate(books)}
    return {
        "meta": {
            "note": "Verse segments come from the dataset's own book/chapter/verse labels. Quotes are found "
                    "automatically by matching four or more consecutive words and are a guide, not an edition.",
            "segments": len(segments),
            "quotes": len(quotes),
        },
        "books": [[b, BOOK_NAMES[b]] for b in books],
        "segments": [[li, index[b], ch, v, s, e] for li, b, ch, v, s, e in segments],
        "quotes": [[li, s, e, index[b], ch, v1, v2, span] for li, s, e, b, ch, v1, v2, span in quotes],
    }

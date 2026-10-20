# Dead-Sea-Scrolls-Hebrew-search
# DSSHS

A small web page for searching the Hebrew and Aramaic text of the Dead Sea Scrolls. It runs on your own computer, needs nothing installed except Python, and works offline once you have the files.

The **Search** tab finds every place a Hebrew word occurs, with the surrounding words, the scroll, fragment or column, and line. You can search whole words, words with prefixes, every form of a lexeme, or a pattern; limit the search to certain scrolls; leave out hits that rest on reconstructed text; and download the hits as a spreadsheet. The **Read** tab lets you open any manuscript and read it from beginning to end.

The text comes from the [CACCHT Dead Sea Scrolls dataset](https://github.com/ETCBC/dss) (ETCBC), covering about 390,000 words in 1,001 scrolls, biblical and non-biblical.

## Quick start

You need Python 3.8 or newer.

```
git clone <this repository>
cd dss-hebrew-search
python serve.py
```

Your browser opens at <http://localhost:8000>. Press Ctrl+C in the terminal to stop.

Do not open `index.html` by double-clicking it. Browsers block a page from reading local data files, so the page needs the small server that `serve.py` starts. The server listens on your own computer only.

<img width="1929" height="967" alt="Screen Shot 2026-10-20 at 10 52 32 AM" src="https://github.com/user-attachments/assets/774994ed-d1d9-4990-964d-05d9caa9e40d" />


## Using the search

| Match | Finds | Example: `עין` |
| --- | --- | --- |
| Whole word | the word exactly as written | עין, עיני |
| Whole word, with prefixes | the word with up to three of ו ה ב כ ל מ ש in front | עין, בעין, מעין, העין |
| Lexeme | every form of the word, using the dataset's own lexemes | עין, עיני, עינים, עיניו, עינוהי |
| Starts with / Ends with / Contains | part of a word | |
| Regular expression | a pattern, on the consonants | `^[והבלמ]?עינ` |

- Vowel points and final letters are ignored, so עין and עינ are the same search. If you have no Hebrew keyboard, use the on-screen one.
- The page matches one word at a time. To find two words that sit next to each other, search the first and read the context.
- **Texts** switches between non-biblical scrolls (the default), biblical scrolls and everything.
- **Scrolls** takes names such as `1QS, CD`. A name that matches a scroll exactly selects only that scroll (`1QS` does not include `1QSa`); end a name with `*` to select everything that starts with it (`4Q4*`). You can also click a scroll in "Hits by scroll".
- **Reconstructed text** can leave out hits that rest on an editor's reconstruction. "Skip fully reconstructed words" drops words in which every letter was supplied by an editor. "Skip any word with reconstructed letters" is stricter and also drops words where only some letters were. The page tells you how many matching words were left out, so you can see what the setting removed.
- **Words of context** sets how many words appear on each side; the context continues into neighbouring lines of the same fragment, with `/` marking a line break.
- The address bar keeps your search (`#q=…`), so you can bookmark it or send the link.
- "Download all hits as CSV" saves every hit (not just the ones on screen) in a file that Excel opens correctly with Hebrew.
- Each result's reference is a link that opens the passage in the Read tab, with the line and the word highlighted.

## Reading a text

Open the **Read** tab, find a text in the list (type part of an id or a name, such as `1QS` or `Enoch`), and click it. Named texts are listed first, then the rest of each cave. "Show" switches between non-biblical scrolls (the default), biblical scrolls and everything.

- **View** is either the whole manuscript on one page (scroll through it) or one column or fragment at a time. **Go to**, **Previous** and **Next** move between columns and fragments.
- Each line has its number on the left edge. For biblical manuscripts the verse reference is shown beside it.
- **Hide fully reconstructed words** replaces words that an editor supplied in full with `[…]`, so you read only what survives. Partly reconstructed and uncertain words stay visible and keep their marks (see below).
- **Click a word** to search for every form of its lexeme (or for the exact word, if it has no lexeme). Hover a word to see its lexeme.
- **Search in this text** takes you to the Search tab with the scroll already filled in.
- The address bar keeps your place (`#read=1QS&frag=3`), so you can bookmark a column or send a link to it.

How words are marked in the results and in the reader:

- faded in brackets: reconstructed by an editor
- dotted underline: partly reconstructed
- wavy underline: uncertain letters
- struck through: removed by an editor
- solid underline: corrected

## Things to know before you cite a result

- Check readings and references against Qumran-Digital or the DJD volumes. The dataset's column and line numbers can differ by a line from printed editions, and the reconstructions are the dataset's.
- Lexeme search uses the dataset's lexemes with the vowel points removed, so homographs share an entry. For example, forms of עור meaning "blind", "skin" and "be awake" all appear under one lexeme.
- The original dataset writes prefixes as separate units. This page joins them to the word they belong to, based on the dataset's own spacing.
- Lines that contain no words are not exported (126 of the 52,895 lines in the source).
- "Reconstructed" follows the dataset's own flag for letters supplied by a modern editor. It does not say how secure a reconstruction is.
- Some scrolls are very fragmentary, so the reader shows many short fragments in the order the dataset gives them, not necessarily the order of the original manuscript.

## Data and license

- The **code** is under the MIT license (see `LICENSE`).
- The **data** in `data/dss.json` is derived from the ETCBC/CACCHT dataset, which is licensed **CC BY-NC 4.0**: free to share and adapt with attribution, **not for commercial use**. The MIT license does not cover it. See `data/README.md`, and credit the dataset when you publish results.

If you publish a fork or put it on GitHub Pages, keep the attribution line at the bottom of the page and the non-commercial terms.

## Data format

`data/dss.json` is one JSON object:

```
{
  "meta":      { "source", "url", "ref", "license", "words", "lines" },
  "scrolls":   ["1QS", "1QSa", "CD", ...],
  "lemmas":    ["עין", "ראה", ...],            // lexemes, consonants only
  "lemmaLang": ["h", "h", "a", ...],           // 'h' Hebrew, 'a' Aramaic, 'g' Greek
  "lines": [
    [ scrollIndex, "3", "7",                   // scroll, fragment or column, line
      ["עוונותו", "להביט", ...],               // words, prefixes joined
      [412, 3051, -1, ...],                    // lexeme id per word, -1 if none
      "0000200...",                            // one status character per word (see below)
      0, ""                                    // 1 and "Num 23:5" for biblical lines
    ],
    ...
  ]
}
```

The status character is base 32 (`0-9a-v`) for the sum of: 1 partly reconstructed, 2 reconstructed, 4 uncertain, 8 corrected, 16 removed.

## Rebuilding the data

You only need this to update to a newer version of the dataset or to change what is exported.

```
pip install -r requirements-build.txt
python build_data.py
```

This downloads about 100 MB of Text-Fabric files (into `.cache/`, ignored by git) and writes `data/dss.json` in about a minute. To use a local clone of [ETCBC/dss](https://github.com/ETCBC/dss) instead: `python build_data.py --tf-dir path/to/dss/tf/2.0.1`. To build from a newer commit: `python build_data.py --ref master`.

## Tests

```
node --test                          # search logic (needs Node 18+)
python -m unittest discover tests    # build helpers
```

The same two commands run on GitHub for every push (`.github/workflows/test.yml`).

## Files

| File | Purpose |
| --- | --- |
| `index.html`, `style.css`, `app.js` | the page (Search and Read tabs) |
| `search.js` | the search and reading logic, shared by the page and the tests |
| `serve.py` | local server and browser launcher |
| `build_data.py` | builds `data/dss.json` from the ETCBC dataset |
| `data/dss.json` | the text (built; see Data and license) |
| `data/scroll-names.json` | common names of scrolls, by hand |
| `tests/` | tests |

## Credits

Text: Martin G. Abegg, Jr. (data files); Jarod Jacobs, Martijn Naaijer and Dirk Roorda (the CACCHT/ETCBC dataset). Data is processed with [Text-Fabric](https://github.com/annotation/text-fabric).

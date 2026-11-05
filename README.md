# Dead Sea Scrolls Hebrew search

A program for searching the Hebrew and Aramaic text of the Dead Sea Scrolls. It is **one file with nothing to install** (no Python, no packages, no internet connection needed): download it, start it, and the search page opens in your browser. Everything runs on your own computer.

The **Search** tab finds every place a Hebrew word occurs, with the surrounding words, the scroll, fragment or column, and line. You can search whole words, words with prefixes, every form of a lexeme, or a pattern; limit the search to certain scrolls; leave out hits that rest on reconstructed text; and download the hits as a spreadsheet. The **Read** tab lets you open any manuscript and read it from beginning to end. The **Biblical parallels** tab shows a biblical chapter in every manuscript that has it, verse by verse in columns, and lists the pesharim and other texts that quote it. The **Statistics** tab counts: the most frequent words, how one word is spread over the scrolls, the words that occur near it, and the words that mark some scrolls out from the others. The same counts are available on the command line.

The text comes from the [CACCHT Dead Sea Scrolls dataset](https://github.com/ETCBC/dss) (ETCBC), covering about 390,000 words in 1,001 scrolls, biblical and non-biblical.

## Quick start

### Download and run

Get the file for your computer from the **Releases** page of this repository, unzip it, and start the program inside.

| Your computer | File |
| --- | --- |
| Windows 10 or 11 | `dss-search-…-windows-x64.zip` (start `dss-search.exe`) |
| Mac, Apple Silicon or Intel | `dss-search-…-macos.zip` (start `dss-search`) |
| Linux, 64-bit Intel or AMD | `dss-search-…-linux-x64.tar.gz` (start `dss-search`) |
| Linux, 64-bit ARM (for example a Raspberry Pi 4 or 5) | `dss-search-…-linux-arm64.tar.gz` (start `dss-search`) |

A small window opens, saying `Serving at http://127.0.0.1:8000/`, and your browser opens on that address. **Close the window (or press Ctrl+C in it) to stop.** If you start it a second time while the first is still running, it uses the next free port and says which.

The program is not signed with a paid certificate, so the first time your system may warn you:

- **Windows** shows "Windows protected your PC". Click **More info**, then **Run anyway**.
- **macOS** says the program "cannot be opened because the developer cannot be verified". Hold Control, click the file, choose **Open**, then **Open** again. (Or, in Terminal, once: `xattr -d com.apple.quarantine dss-search`.)
- **Linux**: if the file will not start, make it executable first: `chmod +x dss-search`.

Do not open `index.html` by double-clicking it. Browsers block a page from reading local data files, so the page needs the program, which serves it. The program listens on your own computer only (`127.0.0.1`); nobody else on your network can reach it.

Options, if you start it from a terminal:

```
dss-search                              # start and open the browser
dss-search --port 9000                  # choose the port (default 8000, or the next free one)
dss-search --no-browser                 # do not open a browser window
dss-search --help                       # everything, including the command-line statistics
```

### Build it yourself

You need [Rust](https://rustup.rs) 1.85 or newer. Nothing else.

```
git clone <this repository>
cd dss-hebrew-search
cargo run --release                     # builds, starts the page, opens the browser
```

`cargo build --release` leaves a single self-contained program in `target/release/` (about 10 MB, with the page and all the text inside it). Copy that one file anywhere.

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

## Comparing manuscripts of a biblical chapter

Open the **Biblical parallels** tab, choose a book and a chapter (the list shows how many manuscripts contain each chapter and how many other texts quote it), and the page shows:

- **A table with one row per verse and one column per manuscript.** For Isaiah 61 that is the Great Isaiah Scroll, 1QIsab, and two Cave 4 manuscripts. A dash means the verse is not preserved in that manuscript. Under each verse is its place in the manuscript (for example `51:2`); click it to read that passage in context.
- **Manuscripts** (a list under the controls) lets you hide columns you do not need. **Hide fully reconstructed words** replaces words that an editor supplied in full with `[…]`, so you can compare only what survives.
- **Quoted or echoed in other texts:** pesharim, the Melchizedek text, rewritten Torah and other works, grouped by text, with the quoted words highlighted and a link to read each in context. For Habakkuk 1 this lists the 18 passages of the Pesher Habakkuk that quote it.
- The address bar keeps the chapter (`#parallels=Is&ch=61`), and `&v=2` highlights a verse. A search hit or a line in the Read tab on a biblical manuscript has its reference (for example `Is 61:1`) as a link to this tab.

How quotations are found: the build compares every stretch of non-biblical text with every verse of every biblical manuscript, ignoring vowel letters (ו, י) and final forms so that full and defective spellings match, and keeps stretches of four or more words in a row. A match that fits many verses (a formula such as "the LORD your God") is dropped. This is a heuristic: it finds paraphrase and rewritten Torah as well as exact quotation, it can miss quotations broken by damage or by a different wording, and it does not itself identify the commentary that follows a quotation.

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

## Statistics

Counting words and comparing scrolls is built into the program: there is nothing to switch on. The Statistics tab asks the program for its numbers, so it works when the page is opened through the program, but not from a saved copy of the page or an ordinary web server (there the tab says so and the other tabs work as before).

### In the page

| Analysis | Answers | Needs |
| --- | --- | --- |
| Frequencies | the most common words, with count, per million, number of scrolls, spread and reconstructed count | nothing |
| Distribution | where one word occurs: per scroll or per cave, with count, per million, share of hits | a word |
| Collocations | words that occur near a word more often than chance would give | a word |
| Keyness | words used much more (or less) in some scrolls than in others | target scrolls |

- **Count** is *Lexemes* (every form of a word counted together, using the dataset's lexemes; words without a lexeme are left out) or *Words as written* (final letters made ordinary, as in the Search tab).
- **Texts**, **Only these scrolls**, **Language** and **Reconstructed text** work exactly as in the Search tab, so searching for a word there with the same settings gives the same count. Tests check this against the Search tab's own code.
- **Hide these words** removes words from a list without changing the totals. The dataset gives single letters left over from damaged words (for example ם or א) lexemes of their own, so they can rank high; this is the way to hide them.
- **Smallest count to list** and **Rows** limit the table; the page says when more rows match than are shown.
- In a table, a word links to the Search tab (limited to the same scrolls), and **scrolls** and **near** run the distribution and collocations of that word. A scroll in a distribution links to the Search tab for that scroll, or to **read** it.
- The address bar keeps the question (`#stats=colloc&word=...`), so a link reproduces a table. **Download all rows as CSV** saves every matching row, not only the ones shown (Excel opens it with the Hebrew intact).

### From the command line

```
dss-search stats info                                    # what is in the corpus
dss-search stats freq --top 30 --exclude "ם א"           # most frequent lexemes
dss-search stats dist יחד --by cave                      # where a word occurs
dss-search stats colloc יחד --window 4 --sort logdice    # words near it
dss-search stats keyness --target 1QS                    # words typical of the Community Rule
dss-search stats keyness --target 1QS --direction under  # ...and the words it avoids
dss-search stats export words --out words.csv            # the whole corpus as a table
```

Every command takes the filters `--corpus`, `--scrolls`, `--lang`, `--rec` and `--unit`, the same as the Search tab. Add `--format csv` or `--format json` to print for another program, or `--out FILE` to save (`.csv`, `.tsv`, `.json` or `.ndjson`; a `.csv` opens in Excel with the Hebrew intact). A one-line description of each result goes to the error stream, so piping stays clean. `dss-search stats COMMAND --help` lists the options. `--top 0` shows every row. Hebrew can look reversed in a terminal that does not handle right-to-left text; saved files and the page are not affected.

`export` saves one of four tables of the whole corpus: `words`, `lines`, `lemmas` or `scrolls`.

| Table | One row per | Columns |
| --- | --- | --- |
| `words` | token, in reading order | `word_id`, `line`, `pos`, `scroll`, `scroll_id`, `frag`, `lnum`, `text`, `lemma_id`, `lemma`, `lang`, `bits`, `biblical`, `form`, `is_word`, `form_key`, `rec_part`, `rec_full`, `unc`, `cor`, `rem`, `seg`, `idx` |
| `lines` | line | `line`, `scroll`, `scroll_id`, `frag`, `lnum`, `biblical`, `bible_ref`, `n_words`, `text` |
| `lemmas` | lexeme | `lemma_id`, `lemma`, `lang` |
| `scrolls` | scroll | `scroll`, `scroll_id`, `name`, `cave`, `words` |

`text` is the token as exported, `form` its Hebrew letters only, and `form_key` the same with final letters made ordinary. `is_word` is false for gap markers, the verse-end sign and lacuna markers. `seg` and `idx` place a word in a run of words that really are next to each other (see Collocations below).

### From Rust

The program is also a library (`dss_search`): `Corpus::from_json` loads `data/dss.json`, and `Corpus::frequencies`, `distribution`, `collocations` and `keyness` return an `Answer` with a table and a summary whose `text` describes it. See `src/stats.rs`.

### How the numbers are made

- **What counts as a word:** a token with at least one Hebrew letter. Gaps (`…`), the verse-end sign (`׃`) and lacuna markers (`# [ ]`) are not words. A word that carries a lacuna marker (for example a prefix right before a gap) is kept, with its letters only.
- **Per million** is the rate among the words counted under the current settings.
- **Spread (DP)**, deviation of proportions (Gries 2008), compares where a word occurs with how large each scroll is: 0 means spread in proportion to size, close to 1 means concentrated in a few scrolls.
- **Collocations.** A word is *near* the node word if it is within the chosen number of words on either side, in the same column or fragment. The window continues over line breaks but stops at a gap, a lacuna marker, a verse-end sign or the end of the fragment. Distance counts every word, including words the filters leave out, so skipping reconstructed words does not make their neighbours adjacent. A word near several occurrences is counted once, at its nearest one (the left one if equally near). Each candidate is scored on the table "near the word or not" against "this word or another": **log-likelihood** (G², higher is stronger evidence) and **logDice** (14 + log₂ of 2 × near / (node frequency + word frequency), at most 14, less swayed by very common words). Only words nearer than chance are listed. The node word stands for every lexeme with those letters (Hebrew and Aramaic), as a lexeme search does.
- **Keyness.** The target scrolls are compared with the reference (by default all other scrolls under the same settings; a scroll named in both counts as target). **Log-likelihood** measures how sure the difference is; **log ratio** is log₂ of the word's rate in the target over its rate in the reference (+1 means twice as frequent, −1 half; a word missing from one side is treated as 0.5). The "Only these scrolls" setting is not used here, because the target and reference choose the scrolls.

### Same numbers as the Python version

This program replaced a Python version that used Polars. The statistics were ported line by line and are tested against it: 179 questions (every analysis, with and without each filter, plus invalid input and its error messages) give identical answers, and the four corpus tables exported by both versions are identical byte for byte. Two small differences:

- Where Polars itself does not give the same order twice (rows tied on every sort key, for example one word that exists as both a Hebrew and an Aramaic lexeme with the same count), this version always puts the lexeme with the lower number first.
- Parquet export is gone. Use `.csv` or `.ndjson`; both open in Excel, R, pandas and Polars.

## Things to know before you cite a result

- Check readings and references against Qumran-Digital or the DJD volumes. The dataset's column and line numbers can differ by a line from printed editions, and the reconstructions are the dataset's.
- Lexeme search uses the dataset's lexemes with the vowel points removed, so homographs share an entry. For example, forms of עור meaning "blind", "skin" and "be awake" all appear under one lexeme.
- The original dataset writes prefixes as separate units. This page joins them to the word they belong to, based on the dataset's own spacing.
- Lines that contain no words are not exported (126 of the 52,895 lines in the source).
- The Biblical parallels tab uses the dataset's own book, chapter and verse labels. Fragments that the dataset did not place in a book (for example `1Q4`) are not included, and Esther, Nehemiah and 1 Chronicles do not appear because the dataset has no text for them.
- The parallels tab compares only Dead Sea Scrolls manuscripts: there is no Masoretic Text or Septuagint column.
- "Reconstructed" follows the dataset's own flag for letters supplied by a modern editor. It does not say how secure a reconstruction is.
- The statistics inherit all of the above. Lexeme counts leave out the 46,777 words (15.7%) that have no lexeme in the dataset (use *Words as written* to count them), and statistics on a single scroll rest on small numbers: check the counts, not only the scores, before drawing conclusions. The log-likelihood and log ratio show how large and how sure a difference is, not why it exists.
- Some scrolls are very fragmentary, so the reader shows many short fragments in the order the dataset gives them, not necessarily the order of the original manuscript.

## Data and license

- The **code** is under the MIT license (see `LICENSE.md`).
- The **data** in `data/dss.json` and `data/bible.json` is derived from the ETCBC/CACCHT dataset, which is licensed **CC BY-NC 4.0**: free to share and adapt with attribution, **not for commercial use**. The MIT license does not cover it. See `data/README.md`, and credit the dataset when you publish results. Tables made from it, such as the statistics you download or `dss-search stats export`, are adaptations and carry the same terms.
- The downloadable program has the data built into it, so **the program as downloaded is under the non-commercial terms too**, whatever the license of its code. The release files include `DATA-LICENSE.md` for this reason.

If you publish a fork, a build or a copy of the page, keep the attribution line at the bottom of the page and the non-commercial terms.

## Data format

`data/dss.json` is one JSON object (`data/bible.json`, used by the Parallels tab, is described after it):

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

`data/bible.json`:

```
{
  "books":    [["Gen", "Genesis"], ["Is", "Isaiah"], ...],      // book codes are the dataset's
  "segments": [[line, book, chapter, verse, firstWord, endWord], ...],   // verse text in biblical manuscripts
  "quotes":   [[line, firstWord, endWord, book, chapter, firstVerse, lastVerse, span], ...]   // quotations elsewhere
}
```

`line` is an index into `data.lines` in `dss.json`, and the word numbers are positions in that line. Pieces of one quotation that run over several lines share a `span` number.

The status character is base 32 (`0-9a-v`) for the sum of: 1 partly reconstructed, 2 reconstructed, 4 uncertain, 8 corrected, 16 removed.

## Rebuilding the data

You only need this to update to a newer version of the dataset or to change what is exported. The program already has the data built in.

```
git clone --depth 1 https://github.com/ETCBC/dss
dss-search build --tf-dir dss/tf/2.0.1      # writes data/dss.json and data/bible.json
```

That takes a couple of seconds. It reads the dataset's Text-Fabric (`.tf`) files directly; there is nothing to install. The released program never goes on the internet, so it cannot fetch the dataset for you. If you build the program yourself with `cargo build --release --features download`, `dss-search build` (without `--tf-dir`) downloads the 20 files it needs (about 53 MB, into `.cache/`, which git ignores) and `--ref master` builds from a newer commit.

To use the new files, either run `dss-search --data-dir data` (files in that folder replace the built-in ones, one by one), or rebuild the program with `cargo build --release` so that it has them inside. `--web-dir DIR` does the same for `index.html`, `style.css`, `app.js` and `search.js`, which is handy when you are changing the page.

The build reproduces the data in this repository exactly: CI rebuilds it from the dataset on every push and requires the files to be identical.

## Tests

```
cargo test --release                 # all of the Rust tests
node --test                          # the page's own tests, if the repository has them (Node 18+)
DSS_TF_DIR=dss/tf/2.0.1 cargo test --release --test builder   # rebuild the data and compare
```

| Test | Checks |
| --- | --- |
| unit tests in `src/` | Hebrew helpers, scroll filters and ordering, segments, the `.tf` reader, table and CSV output, the data builder's helpers |
| `tests/golden.rs` | 179 questions asked of the old Python/Polars version (`tests/golden.json`) must get the same answers |
| `tests/page_agrees.rs` | `web/search.js` and the program agree on the Statistics tab's settings, defaults and columns (needs Node; skipped without it) |
| `tests/server.rs` | starts the real program and talks HTTP to it: the page, the data, the API, errors, caching, several requests at once |
| `tests/bible.rs` | the quotation search reproduces `data/bible.json` from its verse segments |
| `tests/builder.rs` | with `DSS_TF_DIR`, rebuilding from the dataset reproduces `data/dss.json` and `data/bible.json` |

`tests/make_golden.py` makes `tests/golden.json` again from the old Python version, if you ever change what the analyses compute. The same tests run on Windows, macOS and Linux for every push (`.github/workflows/ci.yml`).

## Making a release

1. Raise `version` in `Cargo.toml` and commit.
2. `git tag v0.1.0 && git push origin v0.1.0` (the tag is `v` plus the version).
3. GitHub Actions (`.github/workflows/release.yml`) builds the program for Windows, macOS (one file for Apple Silicon and Intel) and Linux (static, so it runs on any distribution; Intel/AMD and ARM), checks that each one starts, and publishes the downloads with `SHA256SUMS.txt` on the Releases page.

A signed Windows program and a notarised Mac program would remove the first-run warnings, but both need paid developer accounts, so they are not done.

## Files

| File | Purpose |
| --- | --- |
| `web/index.html`, `web/style.css`, `web/app.js` | the page (Search, Read, Biblical parallels and Statistics tabs) |
| `web/search.js` | the search, reading, parallels and Statistics-settings logic, shared by the page and the tests |
| `src/main.rs`, `src/commands.rs` | the command line (`dss-search`, `stats`, `build`) |
| `src/server.rs`, `src/assets.rs` | the local server, and the page and data built into the program |
| `src/corpus.rs` | loads `dss.json` into the tables the statistics run on |
| `src/stats.rs`, `src/query.rs` | the four analyses, and how the page's questions reach them |
| `src/table.rs`, `src/hebrew.rs` | table and CSV/JSON output; Hebrew letter handling |
| `src/builder.rs`, `src/tf.rs`, `src/bible.rs` | builds `dss.json` and `bible.json` from the dataset; reads `.tf` files; finds quotations |
| `src/download.rs` | fetches the dataset (only with `--features download`) |
| `data/dss.json`, `data/bible.json` | the text and the biblical-parallels data (built; see Data and license) |
| `data/scroll-names.json` | common names of scrolls, by hand |
| `tests/` | tests |
| `.github/workflows/` | tests on every push; builds the downloads when a version is tagged |

## Ideas for improvement

- Search for two or more adjacent words (phrase search), and count phrases and word pairs (n-grams) in the statistics.
- Statistics by part of speech, gender or verb stem, once those fields are exported; a way to leave out single letters from damaged words.
- Collocation and keyness across a chosen set of texts at once (for example every copy of the Damascus Document).
- Compare a lexeme's frequency in a text with its frequency in a biblical book.
- Highlight the differences between manuscripts in the parallels table, and add the Masoretic Text and Septuagint as comparison columns.
- Show the pesher's commentary lines together with each quotation, not only the quoted words.
- Filters from the dataset's grammar: part of speech, gender, number, verb stem.
- English glosses for lexemes, and a lexeme browser.
- Search by transliteration.
- Links from each hit to Qumran-Digital or to images of the fragment.
- Mark the reconstructed or uncertain part of a word instead of the whole word, and offer the same "skip" setting for uncertain letters.
- Print or export a whole text as plain text or a document.
- Show a text's columns in a facing-page layout, or two texts side by side (for example, parallel copies of the Damascus Document).
- Remember the last text and position between visits.
- Show corrected and removed text in the results.
- Fill in `data/scroll-names.json` (it covers the main compositions only).
- A workflow that rebuilds the data when ETCBC/dss changes.
- Compile the statistics to WebAssembly (the library is already free of operating-system code) so that the whole tool runs from a web page, for example on GitHub Pages, with no download at all.
- Signed and notarised downloads, so that Windows and macOS do not warn on first run.

## Credits

Text: Martin G. Abegg, Jr. (data files); Jarod Jacobs, Martijn Naaijer and Dirk Roorda (the CACCHT/ETCBC dataset), distributed as [Text-Fabric](https://github.com/annotation/text-fabric) files, which `dss-search build` reads directly. The statistics were first written with [Polars](https://pola.rs) in the Python version of this tool, which this program reproduces.

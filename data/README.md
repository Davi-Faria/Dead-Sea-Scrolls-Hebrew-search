# Data

`dss.json` and `bible.json` are compact exports of the **CACCHT Dead Sea Scrolls dataset**
(<https://github.com/ETCBC/dss>), which is based on data files of Martin G. Abegg, Jr.
It was converted by Jarod Jacobs, Martijn Naaijer and Dirk Roorda
(ETCBC / CACCHT project) and is distributed by them under the
**Creative Commons Attribution-NonCommercial 4.0 International** license
(<https://creativecommons.org/licenses/by-nc/4.0/>).

What that means for this repository, and for the `dss-search` program, which has these files built in:

- You may share and adapt `dss.json` with attribution, **for non-commercial purposes only**.
- The MIT license of the code does **not** apply to `dss.json` or `bible.json`. A downloaded copy of the program contains both, so the non-commercial terms apply to it as a whole, even though its code is MIT.
- These files are adapted versions: prefixes are joined to their words, text-critical marks
  are reduced to per-word status flags, and only the fields listed in the README are kept.
- Tables derived from `dss.json` (statistics and exports from the Statistics tab or `dss-search stats`) are adapted versions too: the same attribution and non-commercial terms apply to them.
- Please also credit the dataset when you use search results in publications. The ETCBC/dss
  README explains how to cite it.

`scroll-names.json` is a short hand-written list of common names for scrolls. It is not part
of the dataset; corrections and additions are welcome.

Built from ETCBC/dss commit `ac91a978d630f426e327f88eb0eeb85d6e67210a` (dataset version 2.0.1).

`bible.json` lines up biblical manuscripts by verse (from the dataset's own labels) and lists stretches of other texts that match biblical verses. The matches are found by this project's own comparison and are not part of the original dataset.

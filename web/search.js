/*
 * search.js - the search and reading logic, with no dependence on the page.
 * Works in the browser (window.DSSSearch) and in Node (require('./search.js')),
 * which is how the tests in tests/ run it.
 *
 * Data comes from data/dss.json (see "Data format" in README.md):
 *   data.scrolls   array of scroll names
 *   data.lemmas    array of lexemes (letters only), data.lemmaLang their language ('h' or 'a')
 *   data.lines     [scrollIndex, fragment, line, words[], lemmaIds[], status, biblical, bibleRef]
 *                  Lines are stored in reading order, so each scroll is one run of lines.
 */
(function (root, factory) {
  if (typeof module === 'object' && module.exports) module.exports = factory();
  else root.DSSSearch = factory();
}(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  var FINALS = { '\u05da': '\u05db', '\u05dd': '\u05de', '\u05df': '\u05e0', '\u05e3': '\u05e4', '\u05e5': '\u05e6' };
  var PREFIX_LETTERS = '\u05d5\u05d4\u05d1\u05db\u05dc\u05de\u05e9'; // ו ה ב כ ל מ ש
  var MARKS = /[\u0591-\u05c7]/g; // cantillation and vowel points
  var B32 = '0123456789abcdefghijklmnopqrstuv';

  // Status bits, one character per word in the data (see src/corpus.rs).
  var REC_PART = 1;  // some letters reconstructed by an editor
  var REC_FULL = 2;  // all letters reconstructed by an editor

  var STATUS_LABELS = [
    [1, 'partly reconstructed by an editor'],
    [2, 'reconstructed by an editor'],
    [4, 'uncertain reading'],
    [8, 'corrected'],
    [16, 'removed by an editor']
  ];

  /**
   * Reduce Hebrew text to its consonant letters, with final letters turned into
   * ordinary ones (ן -> נ), so that "עין" matches "עיני" and vowel points never matter.
   */
  function normalizeHebrew(text) {
    var out = '';
    var chars = String(text || '').normalize('NFD');
    for (var i = 0; i < chars.length; i++) {
      var c = chars[i];
      if (c >= '\u05d0' && c <= '\u05ea') out += FINALS[c] || c;
    }
    return out;
  }

  /** Like normalizeHebrew, but leaves regular-expression syntax alone. */
  function normalizeRegexSource(source) {
    return String(source || '').normalize('NFD').replace(MARKS, '').replace(/[\u05da\u05dd\u05df\u05e3\u05e5]/g, function (c) {
      return FINALS[c];
    });
  }

  function statusValue(char) {
    var value = B32.indexOf(char);
    return value < 0 ? 0 : value;
  }

  /** Build lookup tables once after loading the data. */
  function buildIndex(data) {
    var lines = data.lines;
    var nScrolls = data.scrolls.length;
    var total = 0;
    for (var l = 0; l < lines.length; l++) total += lines[l][3].length;

    var norm = new Array(total);
    var lineOf = new Int32Array(total);
    var posOf = new Uint16Array(total);
    var lemmaOf = new Int32Array(total);
    var statusOf = new Uint8Array(total);
    var scrollStart = new Int32Array(nScrolls).fill(-1);
    var scrollEnd = new Int32Array(nScrolls);
    var scrollWords = new Int32Array(nScrolls);
    var scrollBibLines = new Int32Array(nScrolls);

    var wi = 0;
    for (var li = 0; li < lines.length; li++) {
      var line = lines[li];
      var words = line[3];
      var lemmas = line[4];
      var status = line[5];
      var s = line[0];
      if (scrollStart[s] < 0) scrollStart[s] = li;
      scrollEnd[s] = li + 1;
      scrollWords[s] += words.length;
      if (line[6]) scrollBibLines[s] += 1;
      for (var p = 0; p < words.length; p++, wi++) {
        norm[wi] = normalizeHebrew(words[p]);
        lineOf[wi] = li;
        posOf[wi] = p;
        lemmaOf[wi] = lemmas[p];
        statusOf[wi] = statusValue(status.charAt(p));
      }
    }
    return {
      norm: norm,
      lineOf: lineOf,
      posOf: posOf,
      lemmaOf: lemmaOf,
      statusOf: statusOf,
      lemmaNorm: data.lemmas.map(normalizeHebrew),
      scrollStart: scrollStart,
      scrollEnd: scrollEnd,
      scrollWords: scrollWords,
      scrollBibLines: scrollBibLines,
      size: total
    };
  }

  /**
   * Which scrolls does a filter like "1QS, CD, 4Q4*" select?
   * A name that matches a scroll exactly selects only that scroll; a trailing * or a
   * name that matches no scroll exactly selects every scroll starting with it.
   * Returns null for an empty filter (all scrolls).
   */
  function parseScrollFilter(text, scrolls) {
    var tokens = String(text || '').split(/[\s,;]+/).filter(Boolean);
    if (!tokens.length) return null;
    var selected = new Uint8Array(scrolls.length);
    var lower = scrolls.map(function (s) { return s.toLowerCase(); });
    tokens.forEach(function (token) {
      var t = token.toLowerCase();
      var prefix = t.charAt(t.length - 1) === '*';
      if (prefix) t = t.slice(0, -1);
      var exact = !prefix && lower.indexOf(t) !== -1;
      for (var i = 0; i < lower.length; i++) {
        if (exact ? lower[i] === t : lower[i].indexOf(t) === 0) selected[i] = 1;
      }
    });
    return selected;
  }

  /** Turn the user's query and match mode into a function of a word index. */
  function makeMatcher(mode, query, data, idx) {
    if (/\s/.test(String(query).trim())) {
      return { error: 'The search matches one word at a time. To look for two words, search each one separately.' };
    }
    if (mode === 'regex') {
      if (!String(query).trim()) return { error: 'Enter a regular expression.' };
      var rx;
      try { rx = new RegExp(normalizeRegexSource(query)); }
      catch (e) { return { error: 'That regular expression is not valid (' + e.message + ').' }; }
      return { test: function (wi) { return rx.test(idx.norm[wi]); } };
    }
    var q = normalizeHebrew(query);
    if (!q) return { error: 'Enter a Hebrew word.' };
    switch (mode) {
      case 'exact':
        return { test: function (wi) { return idx.norm[wi] === q; } };
      case 'prefixes': {
        var rxp = new RegExp('^[' + PREFIX_LETTERS + ']{0,3}' + q + '$');
        return { test: function (wi) { return rxp.test(idx.norm[wi]); } };
      }
      case 'starts':
        return { test: function (wi) { return idx.norm[wi].indexOf(q) === 0; } };
      case 'ends':
        return { test: function (wi) { var w = idx.norm[wi]; return w.length >= q.length && w.lastIndexOf(q) === w.length - q.length; } };
      case 'contains':
        return { test: function (wi) { return idx.norm[wi].indexOf(q) !== -1; } };
      case 'lexeme': {
        var ids = {};
        var found = 0;
        for (var i = 0; i < idx.lemmaNorm.length; i++) {
          if (idx.lemmaNorm[i] === q) { ids[i] = true; found++; }
        }
        return { test: function (wi) { return ids[idx.lemmaOf[wi]] === true; }, lexemeMatches: found };
      }
      default:
        return { error: 'Unknown match mode: ' + mode };
    }
  }

  /**
   * Run a search.
   * options: { query, mode, corpus: 'nonbib'|'bib'|'all', scrolls: '1QS, CD', lang: 'any'|'h'|'a',
   *            reconstructed: 'include'|'skipfull'|'skipany' }
   *   reconstructed 'skipfull' ignores words that an editor reconstructed entirely;
   *   'skipany' also ignores words with some reconstructed letters.
   * Returns { hits: [wordIndex...], byScroll: [[scrollName, count]...], skippedReconstructed } or { error }.
   */
  function search(data, idx, options) {
    var matcher = makeMatcher(options.mode || 'exact', options.query, data, idx);
    if (matcher.error) return { error: matcher.error };

    var corpus = options.corpus || 'nonbib';
    var lang = options.lang || 'any';
    var recMask = options.reconstructed === 'skipany' ? (REC_PART | REC_FULL)
      : options.reconstructed === 'skipfull' ? REC_FULL : 0;
    var allowed = parseScrollFilter(options.scrolls, data.scrolls);
    if (allowed) {
      var any = false;
      for (var s = 0; s < allowed.length; s++) if (allowed[s]) { any = true; break; }
      if (!any) return { hits: [], byScroll: [], lexemeMatches: matcher.lexemeMatches, skippedReconstructed: 0 };
    }

    var lines = data.lines;
    var hits = [];
    var counts = {};
    var skipped = 0;
    for (var wi = 0; wi < idx.size; wi++) {
      var line = lines[idx.lineOf[wi]];
      if (corpus === 'nonbib' && line[6]) continue;
      if (corpus === 'bib' && !line[6]) continue;
      if (allowed && !allowed[line[0]]) continue;
      if (lang !== 'any') {
        var lemma = idx.lemmaOf[wi];
        var wordLang = lemma >= 0 ? data.lemmaLang[lemma] : 'h';
        if (lang === 'a' ? wordLang !== 'a' : wordLang === 'a') continue;
      }
      if (!matcher.test(wi)) continue;
      if (recMask && (idx.statusOf[wi] & recMask)) { skipped++; continue; }
      hits.push(wi);
      counts[line[0]] = (counts[line[0]] || 0) + 1;
    }
    var byScroll = Object.keys(counts).map(function (k) { return [data.scrolls[k], counts[k]]; });
    byScroll.sort(function (a, b) { return b[1] - a[1] || (a[0] < b[0] ? -1 : 1); });
    return { hits: hits, byScroll: byScroll, lexemeMatches: matcher.lexemeMatches, skippedReconstructed: skipped };
  }

  function wordItem(line, pos) {
    return { t: line[3][pos], s: line[5].charAt(pos) };
  }

  /**
   * The hit word with `n` words on each side. Context continues into the neighbouring
   * lines of the same fragment; { br: true } marks where a line ends.
   */
  function getContext(data, idx, wi, n) {
    var lines = data.lines;
    var li = idx.lineOf[wi];
    var pos = idx.posOf[wi];
    var line = lines[li];
    var sameFragment = function (other) { return other && other[0] === line[0] && other[1] === line[1]; };

    var left = [];
    var cl = li, cp = pos - 1, remaining = n;
    while (remaining > 0) {
      if (cp < 0) {
        var prev = lines[cl - 1];
        if (!sameFragment(prev)) break;
        cl -= 1; cp = prev[3].length - 1;
        left.push({ br: true });
        continue;
      }
      left.push(wordItem(lines[cl], cp));
      cp -= 1; remaining -= 1;
    }
    left.reverse();

    var right = [];
    cl = li; cp = pos + 1; remaining = n;
    while (remaining > 0) {
      if (cp >= lines[cl][3].length) {
        var next = lines[cl + 1];
        if (!sameFragment(next)) break;
        cl += 1; cp = 0;
        right.push({ br: true });
        continue;
      }
      right.push(wordItem(lines[cl], cp));
      cp += 1; remaining -= 1;
    }

    var lemma = idx.lemmaOf[wi];
    return {
      scroll: data.scrolls[line[0]],
      fragment: line[1],
      line: line[2],
      pos: pos,
      ref: data.scrolls[line[0]] + ' ' + line[1] + ':' + line[2],
      bibleRef: line[7] || '',
      lexeme: lemma >= 0 ? data.lemmas[lemma] : '',
      left: left,
      hit: wordItem(line, pos),
      right: right
    };
  }

  /** Lexemes that start with (or else contain) the typed letters, for "did you mean" hints. */
  function suggestLexemes(data, idx, query, limit) {
    var q = normalizeHebrew(query);
    if (!q) return [];
    var starts = [], contains = [];
    for (var i = 0; i < idx.lemmaNorm.length; i++) {
      var n = idx.lemmaNorm[i];
      if (n.indexOf(q) === 0) starts.push(data.lemmas[i]);
      else if (n.indexOf(q) !== -1) contains.push(data.lemmas[i]);
    }
    return starts.concat(contains).slice(0, limit || 12);
  }

  /** Describe a status character from the data (see src/corpus.rs) in words. */
  function statusLabels(char) {
    var value = B32.indexOf(char);
    if (value < 0) return [];
    return STATUS_LABELS.filter(function (entry) { return value & entry[0]; }).map(function (entry) { return entry[1]; });
  }

  function plain(items) {
    return items.map(function (item) { return item.br ? '/' : item.t; }).join(' ');
  }

  /** CSV text (with a byte-order mark so Excel reads the Hebrew) for the given hits. */
  function toCSV(data, idx, hits, contextWords) {
    var rows = [['reference', 'scroll', 'fragment', 'line', 'bible reference', 'word', 'lexeme', 'context']];
    hits.forEach(function (wi) {
      var c = getContext(data, idx, wi, contextWords);
      rows.push([c.ref, c.scroll, c.fragment, c.line, c.bibleRef, c.hit.t, c.lexeme,
        (plain(c.left) + ' [' + c.hit.t + '] ' + plain(c.right)).trim()]);
    });
    var text = rows.map(function (row) {
      return row.map(function (cell) { return '"' + String(cell).replace(/"/g, '""') + '"'; }).join(',');
    }).join('\r\n');
    return '\ufeff' + text + '\r\n';
  }

  // ------------------------------------------------------------------ reading

  /**
   * One entry per scroll that has text, for the list of texts:
   * { index, id, name, words, lines, kind: 'nonbib' | 'bib' | 'mixed' }.
   * `names` is the optional map from scroll id to a common name.
   */
  function scrollList(data, idx, names) {
    names = names || {};
    var out = [];
    for (var s = 0; s < data.scrolls.length; s++) {
      if (idx.scrollStart[s] < 0) continue;
      var lineCount = idx.scrollEnd[s] - idx.scrollStart[s];
      var bibLines = idx.scrollBibLines[s];
      out.push({
        index: s,
        id: data.scrolls[s],
        name: names[data.scrolls[s]] || '',
        words: idx.scrollWords[s],
        lines: lineCount,
        kind: bibLines === 0 ? 'nonbib' : (bibLines === lineCount ? 'bib' : 'mixed')
      });
    }
    return out;
  }

  /**
   * Filter the list of texts. `corpus` is 'nonbib', 'bib' or 'all' (mixed scrolls appear in
   * both of the first two). Every word typed in `text` must occur in the id or the name.
   */
  function filterScrolls(entries, text, corpus) {
    var terms = String(text || '').toLowerCase().split(/\s+/).filter(Boolean);
    return entries.filter(function (entry) {
      if (corpus === 'nonbib' && entry.kind === 'bib') return false;
      if (corpus === 'bib' && entry.kind === 'nonbib') return false;
      var haystack = (entry.id + ' ' + entry.name).toLowerCase();
      return terms.every(function (term) { return haystack.indexOf(term) !== -1; });
    });
  }

  /** Group entries for display: named texts first, then one group per cave, then the rest. */
  function groupScrolls(entries) {
    var named = [], caves = {}, other = [];
    entries.forEach(function (entry) {
      if (entry.name) { named.push(entry); return; }
      var m = /^(\d+)Q/i.exec(entry.id);
      if (m) (caves[m[1]] = caves[m[1]] || []).push(entry);
      else other.push(entry);
    });
    var groups = [];
    if (named.length) groups.push({ title: 'Named texts', items: named });
    Object.keys(caves).sort(function (a, b) { return a - b; }).forEach(function (cave) {
      groups.push({ title: 'Cave ' + cave, items: caves[cave] });
    });
    if (other.length) groups.push({ title: 'Other sites and manuscripts', items: other });
    return groups;
  }

  /** A readable title for a fragment label from the data ('3' -> Column 3, 'f3ii' -> Fragment 3ii). */
  function fragmentTitle(label) {
    label = String(label || '');
    if (/^\d+$/.test(label)) return 'Column ' + label;
    var m = /^f(.+)$/.exec(label);
    if (m) return 'Fragment ' + m[1].replace(/_/g, '\u2013');
    return label ? 'Section ' + label : 'Text';
  }

  /** The columns or fragments of a scroll, in order: [{ label, title, start, end }] (line index range). */
  function getFragments(data, idx, scrollIndex) {
    var out = [];
    var start = idx.scrollStart[scrollIndex];
    if (start < 0) return out;
    var end = idx.scrollEnd[scrollIndex];
    var current = null;
    for (var li = start; li < end; li++) {
      var label = data.lines[li][1];
      if (!current || current.label !== label) {
        current = { label: label, title: fragmentTitle(label), start: li, end: li + 1 };
        out.push(current);
      } else {
        current.end = li + 1;
      }
    }
    return out;
  }

  /**
   * The words of one line for the reading view: [{ t, s, pos, lexeme }].
   * With hideReconstructed, a run of fully reconstructed words becomes one { gap: true }.
   */
  function readerLine(data, li, hideReconstructed) {
    var line = data.lines[li];
    var out = [];
    for (var p = 0; p < line[3].length; p++) {
      var status = line[5].charAt(p);
      if (hideReconstructed && (statusValue(status) & REC_FULL)) {
        if (!out.length || !out[out.length - 1].gap) out.push({ gap: true });
        continue;
      }
      var lemma = line[4][p];
      out.push({ t: line[3][p], s: status, pos: p, lexeme: lemma >= 0 ? data.lemmas[lemma] : '' });
    }
    return out;
  }

  // -------------------------------------------------------- biblical parallels

  /**
   * Tables for the Parallels tab, built from data/bible.json:
   *   segments  [line, book, chapter, verse, first word, end word]
   *   quotes    [line, first word, end word, book, chapter, first verse, last verse, span]
   */
  function buildBibleIndex(bible, data) {
    var byChapter = {};   // 'book:chapter' -> { scroll -> { verse -> [[line, start, end], ...] } }
    var chapters = {};    // book index -> { chapter -> true }
    var quotesBy = {};    // 'book:chapter' -> quote records
    function noteChapter(b, ch) { (chapters[b] = chapters[b] || {})[ch] = true; }
    bible.segments.forEach(function (seg) {
      var scroll = data.lines[seg[0]][0];
      var key = seg[1] + ':' + seg[2];
      var entry = byChapter[key] || (byChapter[key] = {});
      var verses = entry[scroll] || (entry[scroll] = {});
      (verses[seg[3]] = verses[seg[3]] || []).push([seg[0], seg[4], seg[5]]);
      noteChapter(seg[1], seg[2]);
    });
    bible.quotes.forEach(function (q) {
      var key = q[3] + ':' + q[4];
      (quotesBy[key] = quotesBy[key] || []).push(q);
      noteChapter(q[3], q[4]);
    });
    var bookIndex = {};
    bible.books.forEach(function (b, i) { bookIndex[b[0]] = i; });
    return { books: bible.books, bookIndex: bookIndex, byChapter: byChapter, chapters: chapters, quotesBy: quotesBy };
  }

  /** Chapters of a book that have text or quotations: [{ chapter, manuscripts, quotes }] in order. */
  function bibleChapters(bib, bookIndex) {
    var list = Object.keys(bib.chapters[bookIndex] || {}).map(Number).sort(function (a, b) { return a - b; });
    return list.map(function (ch) {
      var key = bookIndex + ':' + ch;
      var spans = {};
      (bib.quotesBy[key] || []).forEach(function (q) { spans[q[7]] = true; });
      return { chapter: ch, manuscripts: Object.keys(bib.byChapter[key] || {}).length, quotes: Object.keys(spans).length };
    });
  }

  /** Order scroll ids by cave number, then naturally (4Q55 before 4Q56 before 4Q100). */
  function compareScrollIds(a, b) {
    var ca = /^(\d+)Q/i.exec(a), cb = /^(\d+)Q/i.exec(b);
    var na = ca ? parseInt(ca[1], 10) : 999, nb = cb ? parseInt(cb[1], 10) : 999;
    if (na !== nb) return na - nb;
    var pa = a.match(/\d+|\D+/g) || [], pb = b.match(/\d+|\D+/g) || [];
    for (var i = 0; i < Math.min(pa.length, pb.length); i++) {
      if (pa[i] === pb[i]) continue;
      var xa = /^\d/.test(pa[i]), xb = /^\d/.test(pb[i]);
      if (xa && xb) return parseInt(pa[i], 10) - parseInt(pb[i], 10);
      return pa[i] < pb[i] ? -1 : 1;
    }
    return pa.length - pb.length;
  }

  /** 'Is 61:1' -> { code: 'Is', chapter: 61, verse: 1 }, or null. */
  function parseBibleRef(ref) {
    var m = /^(\S+)\s+(\d+):(\d+)$/.exec(String(ref || '').trim());
    return m ? { code: m[1], chapter: parseInt(m[2], 10), verse: parseInt(m[3], 10) } : null;
  }

  function shortFragment(label) {
    return label.charAt(0) === 'f' ? label.slice(1).replace(/_/g, '\u2013') : label;
  }

  /** Where a verse sits in a manuscript, for a caption and a link: { label, frag, line }. */
  function whereIs(data, segs) {
    var first = data.lines[segs[0][0]];
    var last = data.lines[segs[segs.length - 1][0]];
    var label = shortFragment(first[1]) + ':' + first[2];
    if (segs.length > 1 && (last[1] !== first[1] || last[2] !== first[2])) {
      label += last[1] === first[1] ? '\u2013' + last[2] : '\u2013' + shortFragment(last[1]) + ':' + last[2];
    }
    return { label: label, frag: first[1], line: first[2], pos: segs[0][1] };
  }

  /**
   * Everything the Parallels tab shows for one chapter:
   * { book, chapter, witnesses: [{ scroll, id, verses, verseList, first, last }], verses: [...], quotes: [...] }
   * `names` is the optional map from scroll id to a common name.
   */
  function chapterParallels(data, bib, bookIndex, chapter, names) {
    names = names || {};
    var key = bookIndex + ':' + chapter;
    var entry = bib.byChapter[key] || {};
    var witnesses = Object.keys(entry).map(function (scroll) {
      var verses = entry[scroll];
      var list = Object.keys(verses).map(Number).sort(function (a, b) { return a - b; });
      var id = data.scrolls[scroll];
      return { scroll: Number(scroll), id: id, name: names[id] || '', verses: verses, verseList: list,
               first: list[0], last: list[list.length - 1] };
    });
    witnesses.sort(function (a, b) { return compareScrollIds(a.id, b.id); });
    var seen = {};
    witnesses.forEach(function (w) { w.verseList.forEach(function (v) { seen[v] = true; }); });
    var verses = Object.keys(seen).map(Number).sort(function (a, b) { return a - b; });

    // Quotations elsewhere, grouped by scroll and then by quotation (span).
    var byScroll = {};
    (bib.quotesBy[key] || []).forEach(function (q) {
      var scroll = data.lines[q[0]][0];
      var spans = byScroll[scroll] || (byScroll[scroll] = {});
      var span = spans[q[7]] || (spans[q[7]] = { span: q[7], v1: q[5], v2: q[6], records: [] });
      span.records.push(q);
    });
    var quotes = Object.keys(byScroll).map(function (scroll) {
      var spans = Object.keys(byScroll[scroll]).map(function (k) { return byScroll[scroll][k]; });
      spans.sort(function (a, b) { return a.records[0][0] - b.records[0][0] || a.records[0][1] - b.records[0][1]; });
      var id = data.scrolls[scroll];
      return { scroll: Number(scroll), id: id, name: names[id] || '', spans: spans };
    });
    quotes.sort(function (a, b) { return compareScrollIds(a.id, b.id); });
    return { book: bib.books[bookIndex], chapter: chapter, witnesses: witnesses, verses: verses, quotes: quotes };
  }

  /**
   * The words of one verse in one manuscript, as [{ t, s, li, pos }] (with { br: true } where a
   * new line starts). With hideReconstructed, a run of fully reconstructed words becomes one { gap: true }.
   */
  function verseItems(data, segs, hideReconstructed) {
    var out = [];
    segs.forEach(function (seg, k) {
      var line = data.lines[seg[0]];
      if (k > 0 && segs[k - 1][0] !== seg[0]) out.push({ br: true });
      for (var p = seg[1]; p < seg[2]; p++) {
        var status = line[5].charAt(p);
        if (hideReconstructed && (statusValue(status) & REC_FULL)) {
          if (!out.length || !out[out.length - 1].gap) out.push({ gap: true });
          continue;
        }
        out.push({ t: line[3][p], s: status, li: seg[0], pos: p });
      }
    });
    return out;
  }

  /**
   * The lines of one quotation with the quoted words marked:
   * { lines: [{ li, frag, line, items: [{ t, s, q }] }], first: { frag, line, pos } }.
   */
  function quoteView(data, records) {
    var byLine = {};
    records.forEach(function (r) { (byLine[r[0]] = byLine[r[0]] || []).push(r); });
    var lis = Object.keys(byLine).map(Number).sort(function (a, b) { return a - b; });
    var lines = lis.map(function (li) {
      var line = data.lines[li];
      var items = line[3].map(function (w, p) { return { t: w, s: line[5].charAt(p), q: false }; });
      byLine[li].forEach(function (r) { for (var p = r[1]; p < r[2]; p++) items[p].q = true; });
      return { li: li, frag: line[1], line: line[2], items: items };
    });
    var firstRecord = byLine[lis[0]][0];
    var firstLine = data.lines[lis[0]];
    return { lines: lines, first: { frag: firstLine[1], line: firstLine[2], pos: firstRecord[1] } };
  }

  // ---------------------------------------------------------------- statistics tab
  // The Statistics tab asks the local dss-search program for its numbers. These are the
  // parts that need no server: which settings each analysis takes, their defaults (kept in step with
  // DEFAULTS in src/query.rs by a test), the query string and page address for a set of settings,
  // and what the columns of the answers mean.

  var STATS_ENUMS = {
    unit: ['lexeme', 'form'],
    corpus: ['nonbib', 'bib', 'all'],
    lang: ['any', 'h', 'a'],
    rec: ['include', 'skipfull', 'skipany'],
    by: ['scroll', 'cave'],
    direction: ['over', 'under', 'both']
  };
  var STATS_COMMON_DEFAULTS = { unit: 'lexeme', corpus: 'nonbib', lang: 'any', rec: 'include' };
  var STATS_TEXT_FIELDS = ['word', 'target', 'reference', 'scrolls', 'exclude'];
  var STATS_NUMBER_LIMITS = { min: [1, 10000000], top: [0, 5000], window: [1, 30] };

  // fields: the settings that matter to the analysis, besides unit, corpus, lang, rec and top.
  var STATS = {
    freq: {
      label: 'Frequencies: most common words',
      fields: ['scrolls', 'min', 'exclude'],
      sorts: [],
      defaults: { min: 1, top: 100 }
    },
    dist: {
      label: 'Distribution: one word by scroll',
      fields: ['word', 'by', 'sort', 'scrolls'],
      sorts: [['count', 'Most occurrences'], ['order', 'Scroll order']],
      defaults: { top: 0, by: 'scroll', sort: 'count' }
    },
    colloc: {
      label: 'Collocations: words near a word',
      fields: ['word', 'window', 'sort', 'scrolls', 'min', 'exclude'],
      sorts: [['ll', 'Log-likelihood'], ['logdice', 'logDice'], ['count', 'Times near the word']],
      defaults: { min: 3, top: 50, window: 4, sort: 'll' }
    },
    keyness: {
      label: 'Keyness: words typical of scrolls',
      fields: ['target', 'reference', 'direction', 'sort', 'min', 'exclude'],
      sorts: [['ll', 'Log-likelihood (how sure)'], ['log_ratio', 'Log ratio (how big)']],
      defaults: { min: 5, top: 50, sort: 'll', direction: 'over' }
    }
  };

  // Every column an answer can contain. kind: int, dec1, dec2 (numbers), he (Hebrew), text.
  var STATS_COLUMNS = {
    rank: { label: '#', kind: 'int' },
    item: { label: 'Word', kind: 'he' },
    lang: { label: 'Lang.', kind: 'text', title: 'h Hebrew, a Aramaic' },
    count: { label: 'Count', kind: 'int', title: 'Number of occurrences' },
    per_million: { label: 'Per million', kind: 'dec1', title: 'Occurrences per million words in these scrolls' },
    scrolls: { label: 'Scrolls', kind: 'int', title: 'Number of scrolls it occurs in' },
    dp: { label: 'Spread', kind: 'dec2', title: 'Deviation of proportions: 0 means spread over the scrolls in proportion to their size, close to 1 means concentrated in a few' },
    reconstructed: { label: 'Reconstructed', kind: 'int', title: 'Occurrences with letters supplied by an editor' },
    left: { label: 'Left', kind: 'int', title: 'Times it stands before the word' },
    right: { label: 'Right', kind: 'int', title: 'Times it stands after the word' },
    freq: { label: 'In all', kind: 'int', title: 'Occurrences in all the scrolls counted' },
    ll: { label: 'Log-likelihood', kind: 'dec1', title: 'Strength of the evidence that this is not chance; higher is stronger' },
    logdice: { label: 'logDice', kind: 'dec1', title: 'Strength of attraction, at most 14; less swayed by very common words' },
    target: { label: 'Target', kind: 'int', title: 'Occurrences in the target scrolls' },
    reference: { label: 'Reference', kind: 'int', title: 'Occurrences in the reference scrolls' },
    target_pm: { label: 'Target per million', kind: 'dec1' },
    reference_pm: { label: 'Reference per million', kind: 'dec1' },
    log_ratio: { label: 'Log ratio', kind: 'dec2', title: 'log2 of how many times more frequent in the target: +1 is twice as frequent, -1 is half as frequent' },
    target_scrolls: { label: 'Target scrolls', kind: 'int' },
    reference_scrolls: { label: 'Reference scrolls', kind: 'int' },
    group: { label: 'Scroll', kind: 'text' },
    name: { label: 'Name', kind: 'text' },
    words: { label: 'Words', kind: 'int', title: 'Words counted in this scroll (or cave)' },
    share: { label: 'Share of hits', kind: 'dec1', title: 'Percent of all occurrences that are here' }
  };

  function hasOwn(object, key) {
    return Object.prototype.hasOwnProperty.call(object, key);
  }

  /** Every setting of an analysis at its default: {unit, corpus, lang, rec, top, ...}. Null if unknown. */
  function statsDefaults(analysis) {
    if (!hasOwn(STATS, analysis)) return null;
    var spec = STATS[analysis];
    var out = {};
    Object.keys(STATS_COMMON_DEFAULTS).forEach(function (k) { out[k] = STATS_COMMON_DEFAULTS[k]; });
    Object.keys(spec.defaults).forEach(function (k) { out[k] = spec.defaults[k]; });
    STATS_TEXT_FIELDS.forEach(function (k) { if (spec.fields.indexOf(k) !== -1) out[k] = ''; });
    return out;
  }

  /** The query string for the server and for the page address: only settings that matter to the analysis. */
  function statsParams(analysis, options) {
    var spec = STATS[analysis];
    var params = new URLSearchParams();
    Object.keys(STATS_COMMON_DEFAULTS).concat(['top'], spec.fields).forEach(function (key) {
      var value = options[key];
      if (value === undefined || value === null) return;
      value = String(value).trim();
      if (value !== '') params.set(key, value);
    });
    return params.toString();
  }

  /**
   * Read settings from a page address (`stats=freq&unit=form...`). Anything missing or not valid falls
   * back to the default. Returns { analysis, options }, or null when the address is not for this tab.
   */
  function parseStatsParams(params) {
    var analysis = params.get('stats');
    var options = statsDefaults(analysis);
    if (!options) return null;
    var spec = STATS[analysis];
    Object.keys(options).forEach(function (key) {
      var raw = params.get(key);
      if (raw === null) return;
      if (hasOwn(STATS_ENUMS, key)) {
        if (STATS_ENUMS[key].indexOf(raw) !== -1) options[key] = raw;
      } else if (key === 'sort') {
        if (spec.sorts.some(function (s) { return s[0] === raw; })) options.sort = raw;
      } else if (hasOwn(STATS_NUMBER_LIMITS, key)) {
        var n = parseInt(raw, 10);
        if (!isNaN(n)) options[key] = Math.max(STATS_NUMBER_LIMITS[key][0], Math.min(STATS_NUMBER_LIMITS[key][1], n));
      } else {
        options[key] = raw;
      }
    });
    return { analysis: analysis, options: options };
  }

  /** What is still missing before an analysis can run, as a message, or '' if nothing. */
  function statsMissing(analysis, options) {
    if (analysis === 'dist' || analysis === 'colloc') {
      var word = String(options.word || '').trim();
      if (!normalizeHebrew(word)) return 'Enter a Hebrew word.';
      if (/\s/.test(word)) return 'Give one word at a time.';
    }
    if (analysis === 'keyness' && !String(options.target || '').trim()) {
      return 'Name at least one scroll for the target, for example 1QS.';
    }
    return '';
  }

  return {
    normalizeHebrew: normalizeHebrew,
    normalizeRegexSource: normalizeRegexSource,
    buildIndex: buildIndex,
    parseScrollFilter: parseScrollFilter,
    search: search,
    getContext: getContext,
    suggestLexemes: suggestLexemes,
    statusLabels: statusLabels,
    toCSV: toCSV,
    scrollList: scrollList,
    filterScrolls: filterScrolls,
    groupScrolls: groupScrolls,
    fragmentTitle: fragmentTitle,
    getFragments: getFragments,
    readerLine: readerLine,
    buildBibleIndex: buildBibleIndex,
    bibleChapters: bibleChapters,
    compareScrollIds: compareScrollIds,
    parseBibleRef: parseBibleRef,
    whereIs: whereIs,
    chapterParallels: chapterParallels,
    verseItems: verseItems,
    quoteView: quoteView,
    statsAnalyses: STATS,
    statsColumns: STATS_COLUMNS,
    statsDefaults: statsDefaults,
    statsParams: statsParams,
    parseStatsParams: parseStatsParams,
    statsMissing: statsMissing
  };
}));

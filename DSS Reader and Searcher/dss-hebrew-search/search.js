/*
 * search.js - the search logic, with no dependence on the page.
 * Works in the browser (window.DSSSearch) and in Node (require('./search.js')),
 * which is how the tests in tests/ run it.
 *
 * Data comes from data/dss.json (see "Data format" in README.md):
 *   data.scrolls   array of scroll names
 *   data.lemmas    array of lexemes (letters only), data.lemmaLang their language ('h' or 'a')
 *   data.lines     [scrollIndex, fragment, line, words[], lemmaIds[], status, biblical, bibleRef]
 */
(function (root, factory) {
  if (typeof module === 'object' && module.exports) module.exports = factory();
  else root.DSSSearch = factory();
}(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  var FINALS = { '\u05da': '\u05db', '\u05dd': '\u05de', '\u05df': '\u05e0', '\u05e3': '\u05e4', '\u05e5': '\u05e6' };
  var PREFIX_LETTERS = '\u05d5\u05d4\u05d1\u05db\u05dc\u05de\u05e9'; // ו ה ב כ ל מ ש
  var MARKS = /[\u0591-\u05c7]/g; // cantillation and vowel points

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

  /** Build lookup tables once after loading the data. */
  function buildIndex(data) {
    var lines = data.lines;
    var total = 0;
    for (var l = 0; l < lines.length; l++) total += lines[l][3].length;

    var norm = new Array(total);
    var lineOf = new Int32Array(total);
    var posOf = new Uint16Array(total);
    var lemmaOf = new Int32Array(total);
    var wi = 0;
    for (var li = 0; li < lines.length; li++) {
      var words = lines[li][3];
      var lemmas = lines[li][4];
      for (var p = 0; p < words.length; p++, wi++) {
        norm[wi] = normalizeHebrew(words[p]);
        lineOf[wi] = li;
        posOf[wi] = p;
        lemmaOf[wi] = lemmas[p];
      }
    }
    return {
      norm: norm,
      lineOf: lineOf,
      posOf: posOf,
      lemmaOf: lemmaOf,
      lemmaNorm: data.lemmas.map(normalizeHebrew),
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
   * options: { query, mode, corpus: 'nonbib'|'bib'|'all', scrolls: '1QS, CD', lang: 'any'|'h'|'a' }
   * Returns { hits: [wordIndex...], byScroll: [[scrollName, count]...] } or { error }.
   */
  function search(data, idx, options) {
    var matcher = makeMatcher(options.mode || 'exact', options.query, data, idx);
    if (matcher.error) return { error: matcher.error };

    var corpus = options.corpus || 'nonbib';
    var lang = options.lang || 'any';
    var allowed = parseScrollFilter(options.scrolls, data.scrolls);
    if (allowed) {
      var any = false;
      for (var s = 0; s < allowed.length; s++) if (allowed[s]) { any = true; break; }
      if (!any) return { hits: [], byScroll: [], lexemeMatches: matcher.lexemeMatches };
    }

    var lines = data.lines;
    var hits = [];
    var counts = {};
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
      hits.push(wi);
      counts[line[0]] = (counts[line[0]] || 0) + 1;
    }
    var byScroll = Object.keys(counts).map(function (k) { return [data.scrolls[k], counts[k]]; });
    byScroll.sort(function (a, b) { return b[1] - a[1] || (a[0] < b[0] ? -1 : 1); });
    return { hits: hits, byScroll: byScroll, lexemeMatches: matcher.lexemeMatches };
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

  /** Describe a status character from the data (see build_data.py) in words. */
  function statusLabels(char) {
    var value = '0123456789abcdefghijklmnopqrstuv'.indexOf(char);
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

  return {
    normalizeHebrew: normalizeHebrew,
    normalizeRegexSource: normalizeRegexSource,
    buildIndex: buildIndex,
    parseScrollFilter: parseScrollFilter,
    search: search,
    getContext: getContext,
    suggestLexemes: suggestLexemes,
    statusLabels: statusLabels,
    toCSV: toCSV
  };
}));

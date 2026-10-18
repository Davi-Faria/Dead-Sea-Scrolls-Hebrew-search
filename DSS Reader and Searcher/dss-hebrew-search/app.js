/*
 * app.js - connects the page to search.js.
 * Loads data/dss.json, runs searches, draws results, keeps the search in the address
 * bar (#q=...) so a link reproduces it, and offers a CSV download.
 */
(function () {
  'use strict';

  var PAGE_SIZE = 100;
  var KEYBOARD = 'אבגדהוזחטיכלמנסעפצקרשת';
  var STATUS_ORDER = '0123456789abcdefghijklmnopqrstuv';

  var $ = function (id) { return document.getElementById(id); };
  var els = {
    form: $('search-form'), q: $('q'), mode: $('mode'), corpus: $('corpus'),
    scrolls: $('scrolls'), lang: $('lang'), ctx: $('ctx'),
    status: $('status'), summary: $('summary'), summaryTitle: $('summary-title'),
    byScroll: $('by-scroll'), scrollList: $('scroll-list'), csv: $('csv'),
    results: $('results'), more: $('more'), keys: $('keys'), source: $('source')
  };
  var controls = [els.q, els.mode, els.corpus, els.scrolls, els.lang, els.ctx, els.form.querySelector('button[type="submit"]')];

  var data = null;
  var index = null;
  var names = {};
  var current = { hits: [], shown: 0, contextWords: 6 };

  // ---------------------------------------------------------------- loading

  function setStatus(text, isError) {
    els.status.textContent = text;
    els.status.className = isError ? 'error' : '';
  }

  function setEnabled(enabled) {
    controls.forEach(function (control) { control.disabled = !enabled; });
  }

  function fetchJSON(url) {
    return fetch(url).then(function (response) {
      if (!response.ok) throw new Error(url + ' returned ' + response.status);
      return response.json();
    });
  }

  function load() {
    setEnabled(false);
    if (location.protocol === 'file:') {
      setStatus('This page cannot read its data when opened as a file. Run "python serve.py" in this folder and use the address it prints.', true);
      return;
    }
    fetchJSON('data/dss.json')
      .then(function (loaded) {
        data = loaded;
        index = DSSSearch.buildIndex(data);
        return fetchJSON('data/scroll-names.json').catch(function () { return {}; });
      })
      .then(function (loadedNames) {
        names = loadedNames;
        setEnabled(true);
        var meta = data.meta;
        setStatus('Ready. ' + meta.words.toLocaleString() + ' words in ' + data.scrolls.length.toLocaleString() + ' scrolls.');
        els.source.textContent = 'Text: ' + meta.source + ' (' + meta.license + ').';
        restoreFromHash();
      })
      .catch(function (error) {
        setStatus('Could not load data/dss.json (' + error.message + '). If you cloned the repository, make sure data/dss.json exists, or build it with "python build_data.py".', true);
      });
  }

  // ------------------------------------------------------------- search state

  function currentOptions() {
    return {
      query: els.q.value,
      mode: els.mode.value,
      corpus: els.corpus.value,
      scrolls: els.scrolls.value,
      lang: els.lang.value
    };
  }

  function contextWords() {
    var n = parseInt(els.ctx.value, 10);
    return isNaN(n) ? 6 : Math.max(0, Math.min(30, n));
  }

  function writeHash(options) {
    var params = new URLSearchParams();
    params.set('q', options.query);
    params.set('mode', options.mode);
    params.set('corpus', options.corpus);
    if (options.scrolls.trim()) params.set('scrolls', options.scrolls.trim());
    if (options.lang !== 'any') params.set('lang', options.lang);
    params.set('ctx', String(contextWords()));
    history.replaceState(null, '', '#' + params.toString());
  }

  function restoreFromHash() {
    if (!location.hash || location.hash.length < 2) return;
    var params = new URLSearchParams(location.hash.slice(1));
    if (!params.get('q')) return;
    els.q.value = params.get('q');
    var setIfValid = function (select, value) {
      if (value && Array.prototype.some.call(select.options, function (o) { return o.value === value; })) select.value = value;
    };
    setIfValid(els.mode, params.get('mode'));
    setIfValid(els.corpus, params.get('corpus'));
    setIfValid(els.lang, params.get('lang'));
    els.scrolls.value = params.get('scrolls') || '';
    if (params.get('ctx')) els.ctx.value = params.get('ctx');
    runSearch();
  }

  // ---------------------------------------------------------------- rendering

  function wordNode(item) {
    if (item.br) {
      var br = document.createElement('span');
      br.className = 'br';
      br.title = 'end of line';
      br.textContent = '/';
      return br;
    }
    var span = document.createElement('span');
    span.textContent = item.t;
    var value = STATUS_ORDER.indexOf(item.s);
    if (value > 0) {
      var classes = [];
      if (value & 2) classes.push('rec');
      if (value & 1) classes.push('rec-part');
      if (value & 4) classes.push('unc');
      if (value & 8) classes.push('cor');
      if (value & 16) classes.push('rem');
      span.className = classes.join(' ');
      span.title = DSSSearch.statusLabels(item.s).join('; ');
    }
    return span;
  }

  function appendWords(parent, items) {
    items.forEach(function (item) {
      parent.appendChild(wordNode(item));
      parent.appendChild(document.createTextNode(' '));
    });
  }

  function hitNode(wordIndex) {
    var c = DSSSearch.getContext(data, index, wordIndex, current.contextWords);
    var li = document.createElement('li');
    li.className = 'hit';

    var ref = document.createElement('div');
    ref.className = 'ref';
    var id = document.createElement('span');
    id.className = 'ref-id';
    id.textContent = c.ref;
    ref.appendChild(id);
    if (names[c.scroll]) {
      var name = document.createElement('span');
      name.className = 'ref-name';
      name.textContent = names[c.scroll];
      ref.appendChild(name);
    }
    if (c.bibleRef) {
      var bible = document.createElement('span');
      bible.className = 'ref-bible';
      bible.textContent = c.bibleRef;
      ref.appendChild(bible);
    }
    if (c.lexeme) {
      var lexeme = document.createElement('span');
      lexeme.className = 'ref-lexeme';
      lexeme.lang = 'he';
      lexeme.dir = 'rtl';
      lexeme.title = 'lexeme';
      lexeme.textContent = c.lexeme;
      ref.appendChild(lexeme);
    }

    var text = document.createElement('p');
    text.className = 'text';
    text.dir = 'rtl';
    text.lang = 'he';
    appendWords(text, c.left);
    var mark = document.createElement('mark');
    mark.appendChild(wordNode(c.hit));
    text.appendChild(mark);
    text.appendChild(document.createTextNode(' '));
    appendWords(text, c.right);

    li.appendChild(ref);
    li.appendChild(text);
    return li;
  }

  function showMore() {
    var end = Math.min(current.hits.length, current.shown + PAGE_SIZE);
    var fragment = document.createDocumentFragment();
    for (var i = current.shown; i < end; i++) fragment.appendChild(hitNode(current.hits[i]));
    els.results.appendChild(fragment);
    current.shown = end;
    var remaining = current.hits.length - end;
    els.more.hidden = remaining <= 0;
    if (remaining > 0) els.more.textContent = 'Show ' + Math.min(PAGE_SIZE, remaining) + ' more (' + remaining.toLocaleString() + ' left)';
  }

  function renderSummary(result, options) {
    var scrollCount = result.byScroll.length;
    els.summaryTitle.textContent = result.hits.length.toLocaleString() + (result.hits.length === 1 ? ' hit' : ' hits') +
      ' in ' + scrollCount.toLocaleString() + (scrollCount === 1 ? ' scroll' : ' scrolls');
    els.scrollList.textContent = '';
    result.byScroll.forEach(function (entry) {
      var item = document.createElement('li');
      var button = document.createElement('button');
      button.type = 'button';
      button.title = (names[entry[0]] ? names[entry[0]] + ': ' : '') + 'search only ' + entry[0];
      button.appendChild(document.createTextNode(entry[0]));
      var count = document.createElement('span');
      count.className = 'count';
      count.textContent = entry[1];
      button.appendChild(count);
      button.addEventListener('click', function () {
        els.scrolls.value = entry[0];
        runSearch();
        if (els.q.scrollIntoView) els.q.scrollIntoView({ block: 'center' });
      });
      item.appendChild(button);
      els.scrollList.appendChild(item);
    });
    els.byScroll.open = scrollCount <= 25;
    els.byScroll.hidden = scrollCount === 0;
    els.csv.hidden = result.hits.length === 0;
    els.summary.hidden = false;
  }

  function explainNoHits(result, options) {
    els.status.className = '';
    els.status.textContent = 'No matches.';
    var hints = [];
    if (options.mode === 'lexeme' && !result.lexemeMatches) {
      var suggestions = DSSSearch.suggestLexemes(data, index, options.query, 10);
      if (suggestions.length) {
        els.status.textContent = 'No lexeme is spelled exactly like that. Did you mean:';
        var box = document.createElement('span');
        box.className = 'suggestions';
        suggestions.forEach(function (lexeme) {
          var button = document.createElement('button');
          button.type = 'button';
          button.lang = 'he';
          button.textContent = lexeme;
          button.addEventListener('click', function () { els.q.value = lexeme; runSearch(); });
          box.appendChild(button);
        });
        els.status.appendChild(box);
        return;
      }
      hints.push('No lexeme matches; try "Contains" or "Whole word, with prefixes".');
    }
    if (options.corpus === 'nonbib') hints.push('Texts is set to non-biblical scrolls only.');
    if (options.scrolls.trim()) hints.push('Scrolls is limited to ' + options.scrolls.trim() + '.');
    if (options.lang !== 'any') hints.push('Language is limited.');
    if (hints.length) els.status.textContent += ' ' + hints.join(' ');
  }

  // -------------------------------------------------------------------- run

  function runSearch() {
    if (!data) return;
    var options = currentOptions();
    current.contextWords = contextWords();
    var started = performance.now();
    var result = DSSSearch.search(data, index, options);
    els.results.textContent = '';
    els.more.hidden = true;
    current.hits = [];
    current.shown = 0;

    if (result.error) {
      els.summary.hidden = true;
      setStatus(result.error, true);
      return;
    }
    current.hits = result.hits;
    writeHash(options);

    if (!result.hits.length) {
      els.summary.hidden = true;
      explainNoHits(result, options);
      return;
    }
    var seconds = ((performance.now() - started) / 1000).toFixed(2);
    setStatus('Searched ' + index.size.toLocaleString() + ' words in ' + seconds + ' s.');
    renderSummary(result, options);
    showMore();
  }

  function downloadCSV() {
    var csv = DSSSearch.toCSV(data, index, current.hits, current.contextWords);
    var url = URL.createObjectURL(new Blob([csv], { type: 'text/csv;charset=utf-8' }));
    var link = document.createElement('a');
    link.href = url;
    link.download = 'dss-search.csv';
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);
  }

  // ------------------------------------------------------------------ wiring

  KEYBOARD.split('').forEach(function (letter) {
    var button = document.createElement('button');
    button.type = 'button';
    button.textContent = letter;
    button.addEventListener('click', function () {
      els.q.setRangeText(letter, els.q.selectionStart, els.q.selectionEnd, 'end');
      els.q.focus();
    });
    els.keys.appendChild(button);
  });

  els.form.addEventListener('submit', function (event) { event.preventDefault(); runSearch(); });
  els.more.addEventListener('click', showMore);
  els.csv.addEventListener('click', downloadCSV);

  load();
}());

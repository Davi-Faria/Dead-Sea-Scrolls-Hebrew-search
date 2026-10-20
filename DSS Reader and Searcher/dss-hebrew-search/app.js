/*
 * app.js - connects the page to search.js.
 * Two tabs: Search (find words) and Read (read a whole manuscript).
 * The address bar keeps the current search or the text being read (#q=... or #read=...),
 * so a link reproduces it and the browser's back button works between tabs.
 */
(function () {
  'use strict';

  var PAGE_SIZE = 100;
  var KEYBOARD = 'אבגדהוזחטיכלמנסעפצקרשת';
  var STATUS_ORDER = '0123456789abcdefghijklmnopqrstuv';

  var $ = function (id) { return document.getElementById(id); };
  var els = {
    form: $('search-form'), q: $('q'), mode: $('mode'), corpus: $('corpus'),
    scrolls: $('scrolls'), lang: $('lang'), rec: $('rec'), ctx: $('ctx'),
    status: $('status'), summary: $('summary'), summaryTitle: $('summary-title'),
    byScroll: $('by-scroll'), scrollList: $('scroll-list'), csv: $('csv'),
    results: $('results'), more: $('more'), keys: $('keys'), source: $('source'),
    textFilter: $('text-filter'), textCorpus: $('text-corpus'), textGroups: $('text-groups'),
    readerEmpty: $('reader-empty'), readerBody: $('reader-body'), readerTitle: $('reader-title'),
    readerMeta: $('reader-meta'), readerView: $('reader-view'), readerJump: $('reader-jump'),
    readerPrev: $('reader-prev'), readerNext: $('reader-next'), readerHideRec: $('reader-hide-rec'),
    readerSearch: $('reader-search'), readerText: $('reader-text')
  };
  var controls = [els.q, els.mode, els.corpus, els.scrolls, els.lang, els.rec, els.ctx,
    els.form.querySelector('button[type="submit"]'), els.textFilter, els.textCorpus];

  var data = null;
  var index = null;
  var names = {};
  var textEntries = [];
  var listButtons = {};
  var current = { hits: [], shown: 0, contextWords: 6 };
  var reader = { scroll: -1, fragments: [], frag: 0, target: null, lineEls: {} };
  var lastSearchHash = '';
  var lastReadHash = '';

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
        textEntries = DSSSearch.scrollList(data, index, names);
        setEnabled(true);
        var meta = data.meta;
        setStatus('Ready. ' + meta.words.toLocaleString() + ' words in ' + data.scrolls.length.toLocaleString() + ' scrolls.');
        els.source.textContent = 'Text: ' + meta.source + ' (' + meta.license + ').';
        renderTextList();
        route();
      })
      .catch(function (error) {
        setStatus('Could not load data/dss.json (' + error.message + '). If you cloned the repository, make sure data/dss.json exists, or build it with "python build_data.py".', true);
      });
  }

  // ------------------------------------------------------------ tabs, routing

  function showTab(name) {
    ['search', 'read'].forEach(function (tab) {
      var selected = tab === name;
      $('tab-' + tab).hidden = !selected;
      var button = $('tab-btn-' + tab);
      button.setAttribute('aria-selected', selected ? 'true' : 'false');
      button.tabIndex = selected ? 0 : -1;
    });
  }

  /** Change the address-bar hash; if it is already that value, just re-route. */
  function setHash(text) {
    if (location.hash.slice(1) === text) route();
    else location.hash = text;
  }

  function goTab(name) {
    setHash(name === 'read' ? (lastReadHash || 'read=') : lastSearchHash);
  }

  function route() {
    var params = new URLSearchParams(location.hash.slice(1));
    if (params.has('read')) {
      showTab('read');
      if (data) applyReadParams(params);
    } else {
      showTab('search');
      if (data) applySearchParams(params);
    }
  }

  Array.prototype.forEach.call(document.querySelectorAll('.tabs button'), function (button) {
    button.addEventListener('click', function () { goTab(button.getAttribute('data-tab')); });
    button.addEventListener('keydown', function (event) {
      if (event.key !== 'ArrowRight' && event.key !== 'ArrowLeft') return;
      var other = button.getAttribute('data-tab') === 'search' ? 'read' : 'search';
      goTab(other);
      $('tab-btn-' + other).focus();
    });
  });
  window.addEventListener('hashchange', route);

  // ------------------------------------------------------------ search state

  function currentOptions() {
    return {
      query: els.q.value,
      mode: els.mode.value,
      corpus: els.corpus.value,
      scrolls: els.scrolls.value,
      lang: els.lang.value,
      reconstructed: els.rec.value
    };
  }

  function contextWords() {
    var n = parseInt(els.ctx.value, 10);
    return isNaN(n) ? 6 : Math.max(0, Math.min(30, n));
  }

  function searchParams(options) {
    var params = new URLSearchParams();
    params.set('q', options.query);
    params.set('mode', options.mode);
    params.set('corpus', options.corpus);
    if (options.scrolls && options.scrolls.trim()) params.set('scrolls', options.scrolls.trim());
    if (options.lang && options.lang !== 'any') params.set('lang', options.lang);
    if (options.reconstructed && options.reconstructed !== 'include') params.set('rec', options.reconstructed);
    params.set('ctx', String(contextWords()));
    return params.toString();
  }

  function setIfValid(select, value) {
    if (value && Array.prototype.some.call(select.options, function (o) { return o.value === value; })) select.value = value;
  }

  function applySearchParams(params) {
    var hasQuery = !!params.get('q');
    if (!hasQuery && !params.has('scrolls')) return;
    // Coming back to the tab with the same search: keep the results (and the scroll position).
    if (hasQuery && params.toString() === lastSearchHash && els.results.children.length) return;
    if (params.has('q')) els.q.value = params.get('q');
    setIfValid(els.mode, params.get('mode'));
    setIfValid(els.corpus, params.get('corpus'));
    setIfValid(els.lang, params.get('lang'));
    setIfValid(els.rec, params.get('rec') || 'include');
    els.scrolls.value = params.get('scrolls') || '';
    if (params.get('ctx')) els.ctx.value = params.get('ctx');
    if (hasQuery) runSearch();
    else els.q.focus();
  }

  // ---------------------------------------------------------- search results

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

  function readHash(c) {
    return '#' + new URLSearchParams({ read: c.scroll, frag: c.fragment, line: c.line, pos: String(c.pos) }).toString();
  }

  function hitNode(wordIndex) {
    var c = DSSSearch.getContext(data, index, wordIndex, current.contextWords);
    var li = document.createElement('li');
    li.className = 'hit';

    var ref = document.createElement('div');
    ref.className = 'ref';
    var id = document.createElement('a');
    id.className = 'ref-id';
    id.href = readHash(c);
    id.title = 'Read this passage in context';
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

  function renderSummary(result) {
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

  function skippedNote(result) {
    var n = result.skippedReconstructed || 0;
    if (!n) return '';
    return ' Left out ' + n.toLocaleString() + (n === 1 ? ' matching word' : ' matching words') + ' because of the reconstructed-text setting.';
  }

  function explainNoHits(result, options) {
    els.status.className = '';
    els.status.textContent = 'No matches.' + skippedNote(result);
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
    lastSearchHash = searchParams(options);
    history.replaceState(null, '', '#' + lastSearchHash);

    if (!result.hits.length) {
      els.summary.hidden = true;
      explainNoHits(result, options);
      return;
    }
    var seconds = ((performance.now() - started) / 1000).toFixed(2);
    setStatus('Searched ' + index.size.toLocaleString() + ' words in ' + seconds + ' s.' + skippedNote(result));
    renderSummary(result);
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

  // --------------------------------------------------------------- text list

  function renderTextList() {
    var entries = DSSSearch.filterScrolls(textEntries, els.textFilter.value, els.textCorpus.value);
    els.textGroups.textContent = '';
    listButtons = {};
    if (!entries.length) {
      var none = document.createElement('p');
      none.className = 'text-none';
      none.textContent = 'No texts match.';
      els.textGroups.appendChild(none);
      return;
    }
    DSSSearch.groupScrolls(entries).forEach(function (group) {
      var heading = document.createElement('h3');
      heading.textContent = group.title;
      var list = document.createElement('ul');
      group.items.forEach(function (entry) {
        var item = document.createElement('li');
        var button = document.createElement('button');
        button.type = 'button';
        button.className = 'text-item';
        var id = document.createElement('span');
        id.className = 'tid';
        id.textContent = entry.id;
        button.appendChild(id);
        if (entry.name) {
          var name = document.createElement('span');
          name.className = 'tname';
          name.textContent = entry.name;
          button.appendChild(name);
        }
        button.title = entry.words.toLocaleString() + ' words';
        if (entry.index === reader.scroll) button.setAttribute('aria-current', 'true');
        button.addEventListener('click', function () {
          setHash(new URLSearchParams({ read: entry.id }).toString());
        });
        listButtons[entry.index] = button;
        item.appendChild(button);
        list.appendChild(item);
      });
      els.textGroups.appendChild(heading);
      els.textGroups.appendChild(list);
    });
  }

  function markSelectedText() {
    Object.keys(listButtons).forEach(function (key) {
      if (Number(key) === reader.scroll) {
        listButtons[key].setAttribute('aria-current', 'true');
        if (listButtons[key].scrollIntoView) listButtons[key].scrollIntoView({ block: 'nearest' });
      } else {
        listButtons[key].removeAttribute('aria-current');
      }
    });
  }

  // ------------------------------------------------------------------ reader

  function lineKey(frag, line) { return frag + '\u0000' + line; }

  function writeReadHash() {
    if (reader.scroll < 0) return;
    var params = { read: data.scrolls[reader.scroll] };
    var fragment = reader.fragments[reader.frag];
    if (fragment && (reader.frag > 0 || reader.view === 'one')) params.frag = fragment.label;
    lastReadHash = new URLSearchParams(params).toString();
    history.replaceState(null, '', '#' + lastReadHash);
  }

  function readerWordNode(word, line, targeted) {
    if (word.gap) {
      var gap = document.createElement('span');
      gap.className = 'gap';
      gap.title = 'reconstructed text hidden';
      gap.textContent = '[\u2026]';
      return gap;
    }
    var span = wordNode({ t: word.t, s: word.s });
    span.classList.add('w');
    span.setAttribute('data-text', word.t);
    span.setAttribute('data-lexeme', word.lexeme);
    var title = [word.lexeme ? 'lexeme ' + word.lexeme : ''].concat(DSSSearch.statusLabels(word.s)).filter(Boolean).join('; ');
    if (title) span.title = title;
    if (targeted) {
      var mark = document.createElement('mark');
      mark.appendChild(span);
      return mark;
    }
    return span;
  }

  function readerLineNode(li, hide) {
    var line = data.lines[li];
    var div = document.createElement('div');
    div.className = 'line';
    var num = document.createElement('span');
    num.className = 'num';
    num.textContent = line[2];
    if (line[7]) {
      var bref = document.createElement('span');
      bref.className = 'bref';
      bref.textContent = line[7];
      num.appendChild(bref);
    }
    var text = document.createElement('p');
    text.className = 'rt';
    var target = reader.target;
    var isTarget = !!target && target.frag === line[1] && target.line === line[2];
    DSSSearch.readerLine(data, li, hide).forEach(function (word) {
      var targeted = isTarget && target.pos !== null && !word.gap && word.pos === target.pos;
      text.appendChild(readerWordNode(word, line, targeted));
      text.appendChild(document.createTextNode(' '));
    });
    div.appendChild(num);
    div.appendChild(text);
    if (isTarget) div.className += ' target';
    reader.lineEls[lineKey(line[1], line[2])] = div;
    return div;
  }

  function firstVisibleLineKey() {
    var keys = Object.keys(reader.lineEls);
    for (var i = 0; i < keys.length; i++) {
      if (reader.lineEls[keys[i]].getBoundingClientRect().bottom > 60) return keys[i];
    }
    return null;
  }

  function renderReader(afterRender) {
    var hide = els.readerHideRec.checked;
    var whole = els.readerView.value === 'whole';
    els.readerText.textContent = '';
    reader.lineEls = {};
    var shown = whole ? reader.fragments : [reader.fragments[reader.frag]];
    var container = document.createDocumentFragment();
    shown.forEach(function (fragment) {
      var section = document.createElement('section');
      section.className = 'frag';
      section.setAttribute('data-frag', fragment.label);
      var heading = document.createElement('h3');
      heading.className = 'frag-title';
      heading.textContent = fragment.title;
      section.appendChild(heading);
      for (var li = fragment.start; li < fragment.end; li++) section.appendChild(readerLineNode(li, hide));
      container.appendChild(section);
    });
    els.readerText.appendChild(container);
    els.readerPrev.disabled = reader.frag <= 0;
    els.readerNext.disabled = reader.frag >= reader.fragments.length - 1;
    if (afterRender) afterRender();
  }

  function sectionFor(fragmentIndex) {
    var label = reader.fragments[fragmentIndex].label;
    var sections = els.readerText.children;
    for (var i = 0; i < sections.length; i++) {
      if (sections[i].getAttribute('data-frag') === label) return sections[i];
    }
    return null;
  }

  function goFragment(fragmentIndex) {
    if (fragmentIndex < 0 || fragmentIndex >= reader.fragments.length) return;
    reader.frag = fragmentIndex;
    reader.target = null;
    els.readerJump.value = String(fragmentIndex);
    if (els.readerView.value === 'one') {
      renderReader(function () { window.scrollTo(0, els.readerText.getBoundingClientRect().top + window.pageYOffset - 200); });
    } else {
      var section = sectionFor(fragmentIndex);
      if (section && section.scrollIntoView) section.scrollIntoView({ block: 'start' });
      els.readerPrev.disabled = fragmentIndex <= 0;
      els.readerNext.disabled = fragmentIndex >= reader.fragments.length - 1;
    }
    writeReadHash();
  }

  function showTarget() {
    var target = reader.target;
    if (!target) return;
    var element = reader.lineEls[lineKey(target.frag, target.line)];
    if (element && element.scrollIntoView) element.scrollIntoView({ block: 'center' });
  }

  function openText(scrollIndex, target) {
    var entry = textEntries.filter(function (e) { return e.index === scrollIndex; })[0];
    // Make sure the text is visible in the list that is being shown.
    if (!DSSSearch.filterScrolls([entry], els.textFilter.value, els.textCorpus.value).length) {
      els.textFilter.value = '';
      els.textCorpus.value = 'all';
    }
    reader.scroll = scrollIndex;
    reader.fragments = DSSSearch.getFragments(data, index, scrollIndex);
    reader.frag = 0;
    reader.target = null;
    if (target && target.frag !== null) {
      for (var i = 0; i < reader.fragments.length; i++) {
        if (reader.fragments[i].label === target.frag) {
          reader.frag = i;
          if (target.line !== null) reader.target = target;
          break;
        }
      }
    }

    var id = data.scrolls[scrollIndex];
    els.readerEmpty.hidden = true;
    els.readerBody.hidden = false;
    els.readerTitle.textContent = id + (entry.name ? ': ' + entry.name : '');
    var kind = entry.kind === 'bib' ? 'Biblical manuscript. ' : (entry.kind === 'mixed' ? 'Contains biblical and non-biblical text. ' : '');
    els.readerMeta.textContent = kind + reader.fragments.length.toLocaleString() +
      (reader.fragments.length === 1 ? ' column or fragment, ' : ' columns or fragments, ') +
      entry.lines.toLocaleString() + ' lines, ' + entry.words.toLocaleString() + ' words.';

    els.readerJump.textContent = '';
    reader.fragments.forEach(function (fragment, i) {
      var option = document.createElement('option');
      option.value = String(i);
      option.textContent = fragment.title;
      els.readerJump.appendChild(option);
    });
    els.readerJump.value = String(reader.frag);

    renderTextList();
    markSelectedText();
    renderReader(function () {
      if (els.readerView.value === 'whole' && (reader.target || reader.frag > 0)) {
        if (reader.target) showTarget();
        else { var section = sectionFor(reader.frag); if (section && section.scrollIntoView) section.scrollIntoView({ block: 'start' }); }
      } else {
        window.scrollTo(0, 0);
      }
    });
    writeReadHash();
  }

  function applyReadParams(params) {
    var id = params.get('read');
    if (!id) {
      reader.scroll = -1;
      els.readerBody.hidden = true;
      els.readerEmpty.hidden = false;
      els.readerEmpty.textContent = 'Choose a text from the list to read it.';
      markSelectedText();
      return;
    }
    var scrollIndex = data.scrolls.indexOf(id);
    if (scrollIndex < 0 || index.scrollStart[scrollIndex] < 0) {
      els.readerBody.hidden = true;
      els.readerEmpty.hidden = false;
      els.readerEmpty.textContent = 'There is no text called "' + id + '" in this dataset.';
      return;
    }
    var pos = params.get('pos');
    var target = {
      frag: params.has('frag') ? params.get('frag') : null,
      line: params.has('line') ? params.get('line') : null,
      pos: pos !== null && pos !== '' && !isNaN(parseInt(pos, 10)) ? parseInt(pos, 10) : null
    };
    // The same text with no new target (for example, coming back to the tab): leave it as it is.
    if (scrollIndex === reader.scroll && !els.readerBody.hidden && target.frag === null) return;
    openText(scrollIndex, target);
  }

  function changeHideReconstructed() {
    var key = els.readerView.value === 'whole' ? firstVisibleLineKey() : null;
    renderReader(function () {
      var element = key && reader.lineEls[key];
      if (element && element.scrollIntoView) element.scrollIntoView({ block: 'start' });
    });
  }

  els.readerJump.addEventListener('change', function () { goFragment(parseInt(els.readerJump.value, 10)); });
  els.readerPrev.addEventListener('click', function () { goFragment(reader.frag - 1); });
  els.readerNext.addEventListener('click', function () { goFragment(reader.frag + 1); });
  els.readerView.addEventListener('change', function () {
    reader.target = null;
    renderReader(function () {
      if (els.readerView.value === 'whole') {
        var section = sectionFor(reader.frag);
        if (section && section.scrollIntoView) section.scrollIntoView({ block: 'start' });
      }
    });
    writeReadHash();
  });
  els.readerHideRec.addEventListener('change', changeHideReconstructed);
  els.readerSearch.addEventListener('click', function () {
    if (reader.scroll < 0) return;
    setHash(new URLSearchParams({ scrolls: data.scrolls[reader.scroll], corpus: 'all' }).toString());
  });
  els.readerText.addEventListener('click', function (event) {
    var word = event.target.closest ? event.target.closest('.w') : null;
    if (!word) return;
    if (window.getSelection && String(window.getSelection())) return; // the reader is selecting text to copy
    var lexeme = word.getAttribute('data-lexeme');
    setHash(searchParams({
      query: lexeme || word.getAttribute('data-text'),
      mode: lexeme ? 'lexeme' : 'exact',
      corpus: 'all',
      scrolls: '',
      lang: 'any',
      reconstructed: 'include'
    }));
  });
  els.textFilter.addEventListener('input', renderTextList);
  els.textCorpus.addEventListener('change', renderTextList);

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

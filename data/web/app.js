/*
 * app.js - connects the page to search.js.
 * Four tabs: Search (find words), Read (read a whole manuscript), Biblical parallels, and Statistics
 * (which asks the local dss-search program for counts).
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
    readerSearch: $('reader-search'), readerText: $('reader-text'),
    parBook: $('par-book'), parChapter: $('par-chapter'), parHideRec: $('par-hide-rec'), parQuotes: $('par-quotes'),
    parMss: $('par-mss'), parMssSummary: $('par-mss-summary'), parMssList: $('par-mss-list'),
    parEmpty: $('par-empty'), parBody: $('par-body'), parTitle: $('par-title'), parMeta: $('par-meta'),
    parTable: $('par-table'), parQuoteSection: $('par-quote-section'), parQuoteList: $('par-quote-list'),
    parQuoteCount: $('par-quote-count'),
    stNotice: $('st-notice'), stForm: $('st-form'), stAnalysis: $('st-analysis'), stWord: $('st-word'),
    stTarget: $('st-target'), stReference: $('st-reference'), stUnit: $('st-unit'), stCorpus: $('st-corpus'),
    stBy: $('st-by'), stWindow: $('st-window'), stDirection: $('st-direction'), stScrolls: $('st-scrolls'),
    stLang: $('st-lang'), stRec: $('st-rec'), stSort: $('st-sort'), stMin: $('st-min'), stTop: $('st-top'),
    stExclude: $('st-exclude'), stRun: $('st-run'), stKeys: $('st-keys'), stStatus: $('st-status'),
    stResult: $('st-result'), stTitle: $('st-title'), stMeta: $('st-meta'), stTable: $('st-table'),
    stCsv: $('st-csv'), stNotes: $('st-notes')
  };
  var controls = [els.q, els.mode, els.corpus, els.scrolls, els.lang, els.rec, els.ctx,
    els.form.querySelector('button[type="submit"]'), els.textFilter, els.textCorpus,
    els.parBook, els.parChapter, els.parHideRec, els.parQuotes];

  var data = null;
  var index = null;
  var names = {};
  var textEntries = [];
  var listButtons = {};
  var current = { hits: [], shown: 0, contextWords: 6 };
  var reader = { scroll: -1, fragments: [], frag: 0, target: null, lineEls: {} };
  var lastSearchHash = '';
  var lastReadHash = '';
  var lastParallelsHash = '';
  var bib = null;                 // index built from data/bible.json (null if the file is missing)
  var par = { book: -1, chapter: null, hidden: {}, verse: null };
  var lastStatsHash = '';
  var TABS = ['search', 'read', 'parallels', 'stats'];

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
      setStatus('This page cannot read its data when opened as a file. Start the dss-search program, which opens it in its own window.', true);
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
        return fetchJSON('data/bible.json').then(function (bible) {
          bib = DSSSearch.buildBibleIndex(bible, data);
        }, function () { bib = null; });
      })
      .then(function () {
        textEntries = DSSSearch.scrollList(data, index, names);
        setEnabled(true);
        var meta = data.meta;
        setStatus('Ready. ' + meta.words.toLocaleString() + ' words in ' + data.scrolls.length.toLocaleString() + ' scrolls.');
        els.source.textContent = 'Text: ' + meta.source + ' (' + meta.license + ').';
        renderTextList();
        fillBooks();
        route();
      })
      .catch(function (error) {
        setStatus('Could not load data/dss.json (' + error.message + '). Close the window and start the dss-search program again.', true);
      });
  }

  // ------------------------------------------------------------ tabs, routing

  function showTab(name) {
    TABS.forEach(function (tab) {
      var selected = tab === name;
      $('tab-' + tab).hidden = !selected;
      var button = $('tab-btn-' + tab);
      button.setAttribute('aria-selected', selected ? 'true' : 'false');
      button.tabIndex = selected ? 0 : -1;
    });
    document.body.classList.toggle('wide', name === 'parallels');
  }

  /** Change the address-bar hash; if it is already that value, just re-route. */
  function setHash(text) {
    if (location.hash.slice(1) === text) route();
    else location.hash = text;
  }

  function goTab(name) {
    setHash(name === 'read' ? (lastReadHash || 'read=')
      : name === 'parallels' ? (lastParallelsHash || 'parallels=')
      : name === 'stats' ? (lastStatsHash || 'stats=' + els.stAnalysis.value) : lastSearchHash);
  }

  function route() {
    var params = new URLSearchParams(location.hash.slice(1));
    if (params.has('stats')) {
      showTab('stats');
      applyStatsParams(params);
    } else if (params.has('parallels')) {
      showTab('parallels');
      if (data) applyParallelsParams(params);
    } else if (params.has('read')) {
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
      var at = TABS.indexOf(button.getAttribute('data-tab'));
      var other = TABS[(at + (event.key === 'ArrowRight' ? 1 : TABS.length - 1)) % TABS.length];
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
    if (c.bibleRef) ref.appendChild(bibleLink(c.bibleRef, 'ref-bible'));
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

  // In the program's own window there is no download manager. The file is handed to the program
  // instead, which asks where to save it. In an ordinary browser the usual download is used.
  function saveThroughProgram(filename, text) {
    if (!(window.ipc && typeof window.ipc.postMessage === 'function')) return false;
    window.ipc.postMessage(JSON.stringify({ type: 'save', filename: filename, text: text }));
    return true;
  }

  function downloadCSV() {
    var csv = DSSSearch.toCSV(data, index, current.hits, current.contextWords);
    if (saveThroughProgram('dss-search.csv', csv)) return;
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
    if (line[7]) num.appendChild(bibleLink(line[7], 'bref'));
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

  // ---------------------------------------------------------------- parallels

  /** A link to the Parallels tab for a reference such as "Is 61:1" (plain text if the tab has no data). */
  function bibleLink(ref, className) {
    var parsed = DSSSearch.parseBibleRef(ref);
    if (!parsed || !bib || bib.bookIndex[parsed.code] === undefined) {
      var plain = document.createElement('span');
      plain.className = className;
      plain.textContent = ref;
      return plain;
    }
    var link = document.createElement('a');
    link.className = className;
    link.href = '#' + new URLSearchParams({ parallels: parsed.code, ch: String(parsed.chapter), v: String(parsed.verse) }).toString();
    link.title = 'Compare the manuscripts and quotations for this chapter';
    link.textContent = ref;
    return link;
  }

  function fillBooks() {
    els.parBook.textContent = '';
    var none = document.createElement('option');
    none.value = '';
    none.textContent = bib ? 'Choose a book' : 'No data (data/bible.json is missing)';
    els.parBook.appendChild(none);
    if (!bib) { els.parChapter.disabled = true; return; }
    bib.books.forEach(function (book, i) {
      var option = document.createElement('option');
      option.value = book[0];
      option.textContent = book[1];
      els.parBook.appendChild(option);
    });
    fillChapters();
  }

  function fillChapters() {
    els.parChapter.textContent = '';
    var none = document.createElement('option');
    none.value = '';
    none.textContent = 'Choose a chapter';
    els.parChapter.appendChild(none);
    var code = els.parBook.value;
    if (!bib || !code) { els.parChapter.disabled = true; return; }
    els.parChapter.disabled = false;
    DSSSearch.bibleChapters(bib, bib.bookIndex[code]).forEach(function (info) {
      var option = document.createElement('option');
      option.value = String(info.chapter);
      var bits = [];
      if (info.manuscripts) bits.push(info.manuscripts + (info.manuscripts === 1 ? ' manuscript' : ' manuscripts'));
      if (info.quotes) bits.push(info.quotes + (info.quotes === 1 ? ' quotation' : ' quotations'));
      option.textContent = info.chapter + (bits.length ? ' (' + bits.join(', ') + ')' : '');
      els.parChapter.appendChild(option);
    });
  }

  function parallelsHash() {
    var params = { parallels: els.parBook.value, ch: els.parChapter.value };
    if (par.verse) params.v = String(par.verse);
    return new URLSearchParams(params).toString();
  }

  function applyParallelsParams(params) {
    var code = params.get('parallels');
    if (!bib || !code || bib.bookIndex[code] === undefined) {
      par.chapter = null;
      els.parBody.hidden = true;
      els.parEmpty.hidden = false;
      els.parEmpty.textContent = !bib ? 'The parallels data (data/bible.json) is missing.'
        : (code ? 'There is no biblical text called "' + code + '" in this dataset.' : 'Choose a book and chapter to compare manuscripts and see where other texts quote it.');
      if (bib && els.parBook.value !== (code || '')) { els.parBook.value = ''; fillChapters(); }
      return;
    }
    if (els.parBook.value !== code) { els.parBook.value = code; fillChapters(); }
    var chapter = parseInt(params.get('ch'), 10);
    if (isNaN(chapter)) {
      els.parChapter.value = '';
      par.chapter = null;
      els.parBody.hidden = true;
      els.parEmpty.hidden = false;
      els.parEmpty.textContent = 'Choose a chapter.';
      return;
    }
    els.parChapter.value = String(chapter);
    var verse = parseInt(params.get('v'), 10);
    var changed = par.book !== bib.bookIndex[code] || par.chapter !== chapter;
    par.book = bib.bookIndex[code];
    par.chapter = chapter;
    par.verse = isNaN(verse) ? null : verse;
    if (changed) par.hidden = {};
    lastParallelsHash = parallelsHash();
    history.replaceState(null, '', '#' + lastParallelsHash);
    renderParallels(true);
  }

  function parWord(item) {
    if (item.gap) {
      var gap = document.createElement('span');
      gap.className = 'gap';
      gap.title = 'reconstructed text hidden';
      gap.textContent = '[\u2026]';
      return gap;
    }
    return wordNode(item);
  }

  function readLink(id, frag, line, pos, text, title) {
    var link = document.createElement('a');
    link.className = 'where';
    var params = { read: id };
    if (frag !== undefined && frag !== null) { params.frag = frag; params.line = line; params.pos = String(pos); }
    link.href = '#' + new URLSearchParams(params).toString();
    link.title = title || 'Read this in context';
    link.textContent = text;
    return link;
  }

  function renderParallels(scrollToVerse) {
    var info = DSSSearch.chapterParallels(data, bib, par.book, par.chapter, names);
    var hide = els.parHideRec.checked;
    var bookName = info.book[1];
    els.parEmpty.hidden = true;
    els.parBody.hidden = false;
    els.parTitle.textContent = bookName + ' ' + info.chapter;
    var count = info.witnesses.length;
    els.parMeta.textContent = count + (count === 1 ? ' manuscript in the dataset contains' : ' manuscripts in the dataset contain') + ' this chapter' +
      (info.quotes.length ? ', and ' + info.quotes.length + (info.quotes.length === 1 ? ' other text quotes' : ' other texts quote') + ' it.' : '.');

    // manuscript checkboxes
    els.parMssList.textContent = '';
    info.witnesses.forEach(function (w) {
      var label = document.createElement('label');
      var box = document.createElement('input');
      box.type = 'checkbox';
      box.checked = !par.hidden[w.scroll];
      box.addEventListener('change', function () {
        if (box.checked) delete par.hidden[w.scroll]; else par.hidden[w.scroll] = true;
        renderParallels(false);
      });
      label.appendChild(box);
      label.appendChild(document.createTextNode(' ' + w.id + (w.name ? ' (' + w.name + ')' : '')));
      els.parMssList.appendChild(label);
    });
    els.parMss.hidden = info.witnesses.length < 2;
    var visible = info.witnesses.filter(function (w) { return !par.hidden[w.scroll]; });
    els.parMssSummary.textContent = 'Manuscripts shown: ' + visible.length + ' of ' + info.witnesses.length;

    // table
    els.parTable.textContent = '';
    if (!visible.length) {
      var emptyRow = els.parTable.insertRow();
      emptyRow.insertCell().textContent = info.witnesses.length ? 'No manuscripts selected.' : 'No manuscript in the dataset has text of this chapter.';
    } else {
      var seen = {};
      visible.forEach(function (w) { w.verseList.forEach(function (v) { seen[v] = true; }); });
      var verses = Object.keys(seen).map(Number).sort(function (a, b) { return a - b; });
      var head = els.parTable.createTHead().insertRow();
      var corner = document.createElement('th');
      corner.scope = 'col';
      corner.textContent = 'Verse';
      head.appendChild(corner);
      visible.forEach(function (w) {
        var th = document.createElement('th');
        th.scope = 'col';
        var id = document.createElement('a');
        id.className = 'par-id';
        id.href = '#' + new URLSearchParams({ read: w.id }).toString();
        id.title = 'Open this manuscript in the Read tab';
        id.textContent = w.id;
        th.appendChild(id);
        if (w.name) { var nm = document.createElement('span'); nm.className = 'par-name'; nm.textContent = w.name; th.appendChild(nm); }
        var cov = document.createElement('span');
        cov.className = 'par-cov';
        cov.textContent = w.first === w.last ? 'v. ' + w.first : 'vv. ' + w.first + '\u2013' + w.last;
        th.appendChild(cov);
        head.appendChild(th);
      });
      var tbody = els.parTable.createTBody();
      var targetRow = null;
      verses.forEach(function (v) {
        var row = tbody.insertRow();
        var th = document.createElement('th');
        th.scope = 'row';
        th.textContent = v;
        row.appendChild(th);
        if (par.verse === v) { row.className = 'target'; targetRow = row; }
        visible.forEach(function (w) {
          var cell = row.insertCell();
          var segs = w.verses[v];
          if (!segs) { cell.className = 'absent'; cell.textContent = '\u2014'; cell.title = 'Not preserved in ' + w.id; return; }
          var text = document.createElement('p');
          text.className = 'rt';
          text.dir = 'rtl';
          text.lang = 'he';
          DSSSearch.verseItems(data, segs, hide).forEach(function (item) {
            text.appendChild(parWord(item));
            text.appendChild(document.createTextNode(' '));
          });
          cell.appendChild(text);
          var where = DSSSearch.whereIs(data, segs);
          cell.appendChild(readLink(w.id, where.frag, where.line, where.pos, where.label));
        });
      });
      if (scrollToVerse && targetRow && targetRow.scrollIntoView) targetRow.scrollIntoView({ block: 'center' });
    }

    renderQuotes(info);
  }

  function renderQuotes(info) {
    els.parQuoteList.textContent = '';
    var show = els.parQuotes.checked && info.quotes.length;
    els.parQuoteSection.hidden = !els.parQuotes.checked;
    if (!els.parQuotes.checked) return;
    if (!info.quotes.length) {
      els.parQuoteCount.textContent = 'No quotation of this chapter was found in the non-biblical texts.';
      return;
    }
    var total = info.quotes.reduce(function (n, q) { return n + q.spans.length; }, 0);
    els.parQuoteCount.textContent = total + (total === 1 ? ' passage' : ' passages') + ' in ' + info.quotes.length +
      (info.quotes.length === 1 ? ' text' : ' texts') + '.';
    info.quotes.forEach(function (group, index) {
      var details = document.createElement('details');
      details.open = index === 0 || group.spans.length <= 3;
      var summary = document.createElement('summary');
      summary.textContent = group.id + (group.name ? ' (' + group.name + ')' : '') + ': ' + group.spans.length +
        (group.spans.length === 1 ? ' passage' : ' passages');
      details.appendChild(summary);
      group.spans.forEach(function (span) {
        var view = DSSSearch.quoteView(data, span.records);
        var box = document.createElement('div');
        box.className = 'qspan';
        var label = document.createElement('div');
        label.className = 'qlabel';
        label.appendChild(document.createTextNode(info.book[1] + ' ' + info.chapter + ':' + (span.v1 === span.v2 ? span.v1 : span.v1 + '\u2013' + span.v2) + '  '));
        label.appendChild(readLink(group.id, view.first.frag, view.first.line, view.first.pos, 'read in context'));
        box.appendChild(label);
        view.lines.forEach(function (line) {
          var row = document.createElement('div');
          row.className = 'qline';
          var num = document.createElement('span');
          num.className = 'num';
          num.textContent = line.frag.replace(/^f/, '') + ':' + line.line;
          var text = document.createElement('p');
          text.className = 'rt';
          text.dir = 'rtl';
          text.lang = 'he';
          line.items.forEach(function (item) {
            var node = wordNode(item);
            if (item.q) node.classList.add('q');
            text.appendChild(node);
            text.appendChild(document.createTextNode(' '));
          });
          row.appendChild(num);
          row.appendChild(text);
          box.appendChild(row);
        });
        details.appendChild(box);
      });
      els.parQuoteList.appendChild(details);
    });
  }

  els.parBook.addEventListener('change', function () {
    par.verse = null;
    fillChapters();
    setHash(els.parBook.value ? new URLSearchParams({ parallels: els.parBook.value }).toString() : 'parallels=');
  });
  els.parChapter.addEventListener('change', function () {
    par.verse = null;
    if (!els.parChapter.value) { setHash(new URLSearchParams({ parallels: els.parBook.value }).toString()); return; }
    setHash(parallelsHash());
  });
  els.parHideRec.addEventListener('change', function () { if (par.chapter !== null) renderParallels(false); });
  els.parQuotes.addEventListener('change', function () { if (par.chapter !== null) renderParallels(false); });

  // ------------------------------------------------------------- statistics
  // The numbers come from the local dss-search program at api/stats/<analysis>. This tab
  // only collects settings, asks, and shows the table; the settings, defaults and column meanings
  // are in search.js so that they can be tested.

  var STATS = DSSSearch.statsAnalyses;
  var statsState = { api: 'unknown', checking: false, ticket: 0 };
  var stInputs = {
    unit: els.stUnit, corpus: els.stCorpus, lang: els.stLang, rec: els.stRec, top: els.stTop, min: els.stMin,
    exclude: els.stExclude, word: els.stWord, by: els.stBy, sort: els.stSort, window: els.stWindow,
    target: els.stTarget, reference: els.stReference, direction: els.stDirection, scrolls: els.stScrolls
  };
  var stKeyTarget = els.stWord;

  function setStStatus(text, isError) {
    els.stStatus.textContent = text;
    els.stStatus.className = isError ? 'error' : '';
  }

  function statsOptions() {
    var options = {};
    Object.keys(stInputs).forEach(function (key) { options[key] = stInputs[key].value; });
    return options;
  }

  function setStatsInput(key, value) {
    var input = stInputs[key];
    if (!input || value === undefined || value === null) return;
    if (input.tagName === 'SELECT' && !Array.prototype.some.call(input.options, function (o) { return o.value === String(value); })) {
      if (key !== 'top') return;
      var extra = document.createElement('option');   // a row count that came from a link
      extra.value = String(value);
      extra.textContent = value === 0 ? 'All' : String(value);
      input.appendChild(extra);
    }
    input.value = String(value);
  }

  /** Show the settings that belong to an analysis, fill its sort choices, and (optionally) reset its numbers. */
  function applyAnalysis(analysis, resetDefaults) {
    var spec = STATS[analysis];
    els.stAnalysis.value = analysis;
    Array.prototype.forEach.call(document.querySelectorAll('#tab-stats [data-field]'), function (element) {
      element.hidden = !element.getAttribute('data-field').split(' ').some(function (field) {
        return spec.fields.indexOf(field) !== -1;
      });
    });
    var previous = els.stSort.value;
    els.stSort.textContent = '';
    spec.sorts.forEach(function (sort) {
      var option = document.createElement('option');
      option.value = sort[0];
      option.textContent = sort[1];
      els.stSort.appendChild(option);
    });
    if (spec.sorts.some(function (sort) { return sort[0] === previous; })) els.stSort.value = previous;
    if (resetDefaults) {
      Object.keys(spec.defaults).forEach(function (key) { setStatsInput(key, spec.defaults[key]); });
    }
  }

  function statsSearchHash(item, options, scrolls) {
    return '#' + searchParams({
      query: item, mode: options.unit === 'form' ? 'exact' : 'lexeme', corpus: options.corpus,
      scrolls: scrolls, lang: options.lang, reconstructed: options.rec
    });
  }

  function statsHashFor(analysis, word, options, scrolls) {
    var next = DSSSearch.statsDefaults(analysis);
    ['unit', 'corpus', 'lang', 'rec'].forEach(function (key) { next[key] = options[key]; });
    next.word = word;
    next.scrolls = scrolls || '';
    return '#stats=' + analysis + '&' + DSSSearch.statsParams(analysis, next);
  }

  function formatStat(kind, value, column) {
    if (value === null || value === undefined) return '';
    if (kind === 'text' || kind === 'he') return String(value);
    if (kind === 'int') return Number(value).toLocaleString();
    var digits = kind === 'dec2' ? 2 : 1;
    var text = Number(value).toLocaleString(undefined, { minimumFractionDigits: digits, maximumFractionDigits: digits });
    return column === 'log_ratio' && value > 0 ? '+' + text : text;
  }

  function statsLink(href, text) {
    var a = document.createElement('a');
    a.href = href;
    a.textContent = text;
    return a;
  }

  function statsCell(analysis, options, summary, column, value, row, columns) {
    var info = DSSSearch.statsColumns[column] || { kind: 'text' };
    var cell = document.createElement('td');
    var kind = info.kind;
    if (kind === 'he') cell.className = 'st-he';
    else if (kind === 'text') cell.className = column === 'name' ? 'st-text st-dim st-wide' : 'st-text';
    if (kind === 'he') cell.lang = 'he';
    if (column === 'item') {
      var scrolls = analysis === 'keyness' ? options.target : options.scrolls;
      cell.appendChild(statsLink(statsSearchHash(value, options, scrolls), value));
    } else if (column === 'group') {
      var one = summary.by !== 'cave';
      cell.appendChild(statsLink(statsSearchHash(options.word, options, one ? value : value + '*'), value));
    } else {
      cell.textContent = formatStat(kind, value, column);
    }
    if (column === 'log_ratio' && value) cell.className = (cell.className + ' ' + (value > 0 ? 'st-pos' : 'st-neg')).trim();
    if (column === 'share') {
      var bar = document.createElement('span');
      bar.className = 'st-bar';
      bar.setAttribute('aria-hidden', 'true');
      var fill = document.createElement('span');
      fill.style.setProperty('--w', Math.max(0, Math.min(100, value)) + '%');
      bar.appendChild(fill);
      cell.appendChild(bar);
    }
    return cell;
  }

  function statsExploreCell(analysis, options, summary, row, columns) {
    var cell = document.createElement('td');
    cell.className = 'st-explore st-text';
    var item = row[columns.indexOf('item')];
    if (item !== undefined) {
      var scrolls = analysis === 'keyness' ? '' : options.scrolls;
      cell.appendChild(statsLink(statsHashFor('dist', item, options, scrolls), 'scrolls'));
      cell.appendChild(statsLink(statsHashFor('colloc', item, options, scrolls), 'near'));
    } else if (summary.by !== 'cave') {
      cell.appendChild(statsLink('#' + new URLSearchParams({ read: row[0] }).toString(), 'read'));
    }
    return cell;
  }

  function renderStats(analysis, options, body, query) {
    var summary = body.summary || {};
    var columns = body.columns || [];
    var rows = body.rows || [];
    var titles = {
      freq: 'Most frequent words',
      dist: summary.item ? 'Where \u2068' + summary.item + '\u2069 occurs' : 'Distribution',
      colloc: summary.item ? 'Words near \u2068' + summary.item + '\u2069' : 'Collocations',
      keyness: 'Words typical of ' + (options.target || 'the target') +
        (options.reference && options.reference.trim() ? ', compared with ' + options.reference.trim() : '')
    };
    els.stTitle.textContent = titles[analysis];
    // Keep a Hebrew word from reordering the punctuation and numbers around it.
    els.stMeta.textContent = summary.item && summary.text
      ? summary.text.split(summary.item).join('\u2068' + summary.item + '\u2069') : (summary.text || '');
    els.stResult.hidden = false;
    els.stTable.setAttribute('aria-labelledby', 'st-title');
    els.stTable.textContent = '';

    var hasRows = rows.length > 0;
    els.stTable.parentNode.hidden = !hasRows;
    els.stCsv.parentNode.hidden = !hasRows;
    // The download has every matching row, not only the ones shown (like the Search tab's CSV).
    var allRows = Object.assign({}, options, { top: 0 });
    els.stCsv.href = 'api/stats/' + analysis + '?' + DSSSearch.statsParams(analysis, allRows) + '&format=csv';

    if (hasRows) {
      var withExplore = columns.indexOf('item') !== -1 || (columns[0] === 'group' && summary.by !== 'cave');
      var head = document.createElement('tr');
      columns.forEach(function (column) {
        var info = DSSSearch.statsColumns[column] || { label: column, kind: 'text' };
        var th = document.createElement('th');
        th.scope = 'col';
        th.textContent = column === 'group' ? (summary.by === 'cave' ? 'Cave' : 'Scroll') : info.label;
        if (info.title) th.title = info.title;
        if (info.kind === 'he' || info.kind === 'text') th.className = 'st-' + info.kind;
        head.appendChild(th);
      });
      if (withExplore) {
        var more = document.createElement('th');
        more.scope = 'col';
        more.className = 'st-text';
        more.textContent = columns.indexOf('item') !== -1 ? 'Explore' : '';
        head.appendChild(more);
      }
      var thead = document.createElement('thead');
      thead.appendChild(head);
      els.stTable.appendChild(thead);
      var tbody = document.createElement('tbody');
      rows.forEach(function (row) {
        var tr = document.createElement('tr');
        columns.forEach(function (column, i) {
          tr.appendChild(statsCell(analysis, options, summary, column, row[i], row, columns));
        });
        if (withExplore) tr.appendChild(statsExploreCell(analysis, options, summary, row, columns));
        tbody.appendChild(tr);
      });
      els.stTable.appendChild(tbody);
    }

    var notes = (summary.notes || []).slice();
    if (summary.hidden) notes.push(summary.hidden.toLocaleString() + (summary.hidden === 1 ? ' word' : ' words') + ' hidden by your list.');
    if (summary.matching > summary.shown) notes.push('Showing ' + summary.shown.toLocaleString() + ' of ' + summary.matching.toLocaleString() + '. Raise "Rows" in More settings to see more.');
    els.stNotes.textContent = notes.join(' ');

    if (summary.suggestions && summary.suggestions.length) {
      els.stStatus.className = '';
      els.stStatus.textContent = 'Did you mean:';
      var box = document.createElement('span');
      box.className = 'suggestions';
      summary.suggestions.forEach(function (word) {
        var button = document.createElement('button');
        button.type = 'button';
        button.textContent = word;
        button.lang = 'he';
        button.addEventListener('click', function () { els.stWord.value = word; runStats(); });
        box.appendChild(button);
      });
      els.stStatus.appendChild(box);
    } else {
      setStStatus('');
    }
  }

  function showStatsNotice(info) {
    statsState.api = 'unavailable';
    els.stRun.disabled = true;
    els.stNotice.textContent = '';
    var why = info && (info.reason || info.error);
    var reason = why ? why + ' ' : 'The Statistics tab needs the dss-search program, which does the counting on your computer. ';
    els.stNotice.appendChild(document.createTextNode(reason + 'Start '));
    var program = document.createElement('code');
    program.textContent = 'dss-search';
    els.stNotice.appendChild(program);
    els.stNotice.appendChild(document.createTextNode(' (it opens this page in its own window) rather than opening a saved copy of the page. Searching, reading and the parallels do not need it.'));
    els.stNotice.hidden = false;
  }

  function checkStatsApi() {
    if (statsState.api !== 'unknown' || statsState.checking) return;
    statsState.checking = true;
    fetch('api/status')
      .then(function (response) { return response.ok ? response.json() : null; })
      .catch(function () { return null; })
      .then(function (info) {
        statsState.checking = false;
        if (info && info.stats) {
          statsState.api = 'ready';
          els.stNotice.hidden = true;
          els.stRun.disabled = false;
        } else {
          showStatsNotice(info);
        }
      });
  }

  function runStats() {
    var analysis = els.stAnalysis.value;
    var options = statsOptions();
    var missing = DSSSearch.statsMissing(analysis, options);
    if (missing) {
      els.stResult.hidden = true;
      setStStatus(missing, true);
      return;
    }
    if (statsState.api === 'unavailable') return;
    var query = DSSSearch.statsParams(analysis, options);
    var ticket = ++statsState.ticket;
    var started = performance.now();
    setStStatus('Counting\u2026');
    fetch('api/stats/' + analysis + '?' + query)
      .then(function (response) {
        return response.json().then(
          function (body) { return { status: response.status, ok: response.ok, body: body }; },
          function () { return { status: response.status, ok: false, body: null }; });
      })
      .then(function (reply) {
        if (ticket !== statsState.ticket) return;          // a newer question replaced this one
        if (reply.status === 404 && !reply.body || reply.status === 501) {
          els.stResult.hidden = true;
          showStatsNotice(reply.body);
          setStStatus('');
          return;
        }
        if (!reply.ok) {
          els.stResult.hidden = true;
          setStStatus((reply.body && reply.body.error) || 'The server could not answer (' + reply.status + ').', true);
          return;
        }
        lastStatsHash = 'stats=' + analysis + '&' + query;
        history.replaceState(null, '', '#' + lastStatsHash);
        renderStats(analysis, options, reply.body, query);
        if (!els.stStatus.firstChild) {
          setStStatus('Counted in ' + ((performance.now() - started) / 1000).toFixed(2) + ' s.');
        }
      }, function () {
        if (ticket !== statsState.ticket) return;
        els.stResult.hidden = true;
        showStatsNotice(null);
        setStStatus('');
      });
  }

  function applyStatsParams(params) {
    checkStatsApi();
    var parsed = DSSSearch.parseStatsParams(params);
    if (!parsed) return;
    var hash = 'stats=' + parsed.analysis + '&' + DSSSearch.statsParams(parsed.analysis, parsed.options);
    // Coming back to the tab with the same question: keep the table (and the scroll position).
    if (hash === lastStatsHash && els.stTable.rows.length) return;
    applyAnalysis(parsed.analysis, false);
    Object.keys(parsed.options).forEach(function (key) { setStatsInput(key, parsed.options[key]); });
    if (DSSSearch.statsMissing(parsed.analysis, parsed.options)) {
      els.stResult.hidden = true;
      setStStatus('');
      (parsed.analysis === 'keyness' ? els.stTarget : els.stWord).focus();
    } else {
      runStats();
    }
  }

  Object.keys(STATS).forEach(function (key) {
    var option = document.createElement('option');
    option.value = key;
    option.textContent = STATS[key].label;
    els.stAnalysis.appendChild(option);
  });
  applyAnalysis('freq', true);

  els.stAnalysis.addEventListener('change', function () {
    var analysis = els.stAnalysis.value;
    applyAnalysis(analysis, true);
    if (DSSSearch.statsMissing(analysis, statsOptions())) {
      els.stResult.hidden = true;
      setStStatus('');
    } else {
      runStats();
    }
  });
  els.stForm.addEventListener('submit', function (event) { event.preventDefault(); runStats(); });
  [els.stWord, els.stExclude].forEach(function (input) {
    input.addEventListener('focus', function () { stKeyTarget = input; });
  });

  // ------------------------------------------------------------------ wiring

  function buildKeyboard(container, getInput) {
    KEYBOARD.split('').forEach(function (letter) {
      var button = document.createElement('button');
      button.type = 'button';
      button.textContent = letter;
      button.addEventListener('click', function () {
        var input = getInput();
        input.setRangeText(letter, input.selectionStart, input.selectionEnd, 'end');
        input.focus();
      });
      container.appendChild(button);
    });
  }
  buildKeyboard(els.keys, function () { return els.q; });
  buildKeyboard(els.stKeys, function () { return stKeyTarget; });

  els.form.addEventListener('submit', function (event) { event.preventDefault(); runSearch(); });
  els.more.addEventListener('click', showMore);
  els.csv.addEventListener('click', downloadCSV);
  els.stCsv.addEventListener('click', function (event) {
    if (!(window.ipc && typeof window.ipc.postMessage === 'function')) return; // a browser downloads the link itself
    event.preventDefault();
    var href = els.stCsv.href;
    var name = 'dss-' + ((/api\/stats\/([a-z]+)/.exec(href) || [])[1] || 'statistics') + '.csv';
    fetch(href).then(function (response) {
      if (!response.ok) throw new Error('The program answered ' + response.status + '.');
      return response.arrayBuffer();
    }).then(function (bytes) {
      // keep the byte-order mark (text() would drop it), so that Excel reads the Hebrew
      saveThroughProgram(name, new TextDecoder('utf-8', { ignoreBOM: true }).decode(bytes));
    }).catch(function (error) {
      setStStatus('Could not make the file: ' + error.message, true);
    });
  });

  load();
}());

// Run with:  node --test
const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const S = require('../search.js');

// A tiny corpus in the same format as data/dss.json.
// Line: [scrollIndex, fragment, line, words, lemmaIds, status, biblical, bibleRef]
const fixture = {
  scrolls: ['1QS', '1QSa', 'CD', '1QIsaa'],
  lemmas: ['עין', 'ראה', 'חזה', 'חזה'],
  lemmaLang: ['h', 'h', 'h', 'a'],
  lines: [
    [0, '1', '1', ['ועיני', 'זנות', 'בעיניו'], [0, -1, 0], '000', 0, ''],
    [0, '1', '2', ['ראה', 'עין', 'מעין'], [1, 0, -1], '020', 0, ''],
    [1, '2', '1', ['העין', 'חזה'], [0, 2], '00', 0, ''],
    [2, '3', '1', ['חזא', 'עינוהי'], [3, 0], '00', 0, ''],
    [3, '4', '1', ['עיני', 'יהוה'], [0, -1], '00', 1, 'Isa 1:1']
  ]
};
const idx = S.buildIndex(fixture);
const run = (options) => S.search(fixture, idx, options);
const texts = (result) => result.hits.map((wi) => fixture.lines[idx.lineOf[wi]][3][idx.posOf[wi]]);

test('normalizeHebrew drops vowel points and unifies final letters', () => {
  assert.strictEqual(S.normalizeHebrew('עַיִן'), 'עינ');
  assert.strictEqual(S.normalizeHebrew('עין'), 'עינ');
  assert.strictEqual(S.normalizeHebrew('abc ךםןףץ'), 'כמנפצ');
});

test('exact match finds the whole word only', () => {
  assert.deepStrictEqual(texts(run({ query: 'עין', mode: 'exact', corpus: 'all' })), ['עין']);
});

test('exact match is the same whether the final letter is typed in final or medial form', () => {
  const a = run({ query: 'עין', mode: 'exact', corpus: 'all' });
  const b = run({ query: 'עינ', mode: 'exact', corpus: 'all' });
  assert.deepStrictEqual(a.hits, b.hits);
});

test('prefix mode allows up to three prefix letters but no other letters', () => {
  // מעין counts because מ is one of the prefix letters; ועיני and עינוהי do not match.
  assert.deepStrictEqual(texts(run({ query: 'עין', mode: 'prefixes', corpus: 'all' })), ['עין', 'מעין', 'העין']);
});

test('starts, ends and contains', () => {
  assert.deepStrictEqual(texts(run({ query: 'עינ', mode: 'starts', corpus: 'all' })), ['עין', 'עינוהי', 'עיני']);
  assert.deepStrictEqual(texts(run({ query: 'עיניו', mode: 'ends', corpus: 'all' })), ['בעיניו']);
  assert.strictEqual(run({ query: 'עינ', mode: 'contains', corpus: 'all' }).hits.length, 7);
});

test('lexeme mode returns every form of the lexeme', () => {
  const result = run({ query: 'עין', mode: 'lexeme', corpus: 'all' });
  assert.deepStrictEqual(texts(result), ['ועיני', 'בעיניו', 'עין', 'העין', 'עינוהי', 'עיני']);
  assert.strictEqual(result.lexemeMatches, 1);
});

test('regex mode searches the normalized text and reports bad patterns', () => {
  assert.deepStrictEqual(texts(run({ query: '^עי[ןנ]$', mode: 'regex', corpus: 'all' })), ['עין']);
  assert.ok(run({ query: '(', mode: 'regex', corpus: 'all' }).error);
});

test('a query with two words is an error, since words are matched one at a time', () => {
  assert.match(run({ query: 'רע עין', mode: 'exact', corpus: 'all' }).error, /one word at a time/);
  assert.ok(!run({ query: ' עין ', mode: 'exact', corpus: 'all' }).error);
});

test('an empty query is an error', () => {
  assert.ok(run({ query: '  ', mode: 'exact', corpus: 'all' }).error);
  assert.ok(run({ query: '', mode: 'regex', corpus: 'all' }).error);
});

test('corpus filter separates biblical and non-biblical lines', () => {
  assert.strictEqual(run({ query: 'עיני', mode: 'exact', corpus: 'all' }).hits.length, 1);
  assert.strictEqual(run({ query: 'עיני', mode: 'exact', corpus: 'bib' }).hits.length, 1);
  assert.strictEqual(run({ query: 'עיני', mode: 'exact', corpus: 'nonbib' }).hits.length, 0);
  assert.strictEqual(run({ query: 'עין', mode: 'exact', corpus: 'nonbib' }).hits.length, 1);
});

test('scroll filter: exact name, wildcard, case, and prefix fallback', () => {
  assert.deepStrictEqual(Array.from(S.parseScrollFilter('1QS', fixture.scrolls)), [1, 0, 0, 0]);
  assert.deepStrictEqual(Array.from(S.parseScrollFilter('1QS*', fixture.scrolls)), [1, 1, 0, 0]);
  assert.deepStrictEqual(Array.from(S.parseScrollFilter('1qsa, cd', fixture.scrolls)), [0, 1, 1, 0]);
  assert.deepStrictEqual(Array.from(S.parseScrollFilter('1QI', fixture.scrolls)), [0, 0, 0, 1]);
  assert.strictEqual(S.parseScrollFilter('', fixture.scrolls), null);
  assert.deepStrictEqual(run({ query: 'עין', mode: 'lexeme', corpus: 'all', scrolls: 'XYZ' }).hits, []);
});

test('language filter uses the language of the lexeme', () => {
  assert.deepStrictEqual(texts(run({ query: 'חזא', mode: 'exact', corpus: 'all', lang: 'a' })), ['חזא']);
  assert.deepStrictEqual(texts(run({ query: 'חזא', mode: 'exact', corpus: 'all', lang: 'h' })), []);
});

test('hits are grouped by scroll, most hits first', () => {
  const result = run({ query: 'עינ', mode: 'contains', corpus: 'all' });
  assert.deepStrictEqual(result.byScroll[0], ['1QS', 4]);
});

test('context continues into the previous and next line of the same fragment', () => {
  const hit = run({ query: 'ראה', mode: 'exact', corpus: 'all' }).hits[0];
  const c = S.getContext(fixture, idx, hit, 4);
  assert.strictEqual(c.ref, '1QS 1:2');
  assert.deepStrictEqual(c.left.map((x) => (x.br ? '/' : x.t)), ['ועיני', 'זנות', 'בעיניו', '/']);
  assert.deepStrictEqual(c.right.map((x) => x.t), ['עין', 'מעין']);
  assert.strictEqual(c.lexeme, 'ראה');
});

test('context does not cross into a different fragment', () => {
  const hit = run({ query: 'חזה', mode: 'exact', corpus: 'all' }).hits[0];
  const c = S.getContext(fixture, idx, hit, 5);
  assert.deepStrictEqual(c.left.map((x) => x.t), ['העין']);
  assert.deepStrictEqual(c.right, []);
});

test('biblical lines carry their Bible reference', () => {
  const hit = run({ query: 'עיני', mode: 'exact', corpus: 'bib' }).hits[0];
  assert.strictEqual(S.getContext(fixture, idx, hit, 1).bibleRef, 'Isa 1:1');
});

test('statusLabels decodes the status characters', () => {
  assert.deepStrictEqual(S.statusLabels('0'), []);
  assert.deepStrictEqual(S.statusLabels('2'), ['reconstructed by an editor']);
  assert.deepStrictEqual(S.statusLabels('5'), ['partly reconstructed by an editor', 'uncertain reading']);
});

test('CSV has a header, quoted cells, and a byte-order mark', () => {
  const hits = run({ query: 'עין', mode: 'exact', corpus: 'all' }).hits;
  const csv = S.toCSV(fixture, idx, hits, 2);
  assert.ok(csv.startsWith('\ufeff"reference"'));
  assert.ok(csv.includes('"1QS 1:2"'));
});

test('suggestLexemes finds lexemes by their beginning', () => {
  assert.deepStrictEqual(S.suggestLexemes(fixture, idx, 'חז', 5), ['חזה', 'חזה']);
});

// Checks against the real data. Skipped if data/dss.json has not been built.
const dataFile = path.join(__dirname, '..', 'data', 'dss.json');
test('real data: known passages can be found', { skip: !fs.existsSync(dataFile) }, () => {
  const data = JSON.parse(fs.readFileSync(dataFile, 'utf8'));
  const real = S.buildIndex(data);

  const eye = S.search(data, real, { query: 'עין', mode: 'exact', corpus: 'nonbib', scrolls: '4Q424' });
  assert.ok(eye.hits.length >= 1, 'עין occurs in 4Q424');
  assert.ok(S.getContext(data, real, eye.hits[0], 3).ref.startsWith('4Q424'));

  const gaze = S.search(data, real, { query: 'נבט', mode: 'lexeme', corpus: 'nonbib', scrolls: '1QS' });
  assert.ok(gaze.hits.length >= 5, 'forms of נבט occur in 1QS');
  assert.ok(gaze.hits.some((wi) => real.norm[wi] === 'להביט'), 'להביט (1QS 3:7) is found');

  const withPrefixes = S.search(data, real, { query: 'עין', mode: 'prefixes', corpus: 'nonbib' });
  assert.ok(withPrefixes.hits.length > 20);
});

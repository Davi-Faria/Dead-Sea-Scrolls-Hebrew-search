// Opens the real page against a running `dss-search serve --port N` in a simulated browser (jsdom)
// and checks the four Statistics analyses, the suggestions for an unknown word and the data load.
//
//   npm install            (once, in this folder)
//   dss-search serve --port 8199 &
//   node pages.check.js 8199

const { JSDOM, VirtualConsole } = require('jsdom');
const port = process.argv[2];
const base = `http://127.0.0.1:${port}/`;

async function open(hash, waitFor, timeout = 15000) {
  const errors = [];
  const vc = new VirtualConsole();
  vc.on('jsdomError', (e) => errors.push('jsdomError: ' + (e.detail || e.message)));
  vc.on('error', (e) => errors.push('console.error: ' + e));
  const dom = await JSDOM.fromURL(base + hash, {
    runScripts: 'dangerously', resources: 'usable', pretendToBeVisual: true, virtualConsole: vc,
    beforeParse(window) {
      window.fetch = (url, opts) => fetch(new URL(url, window.location.href), opts);
    },
  });
  const t0 = Date.now();
  while (Date.now() - t0 < timeout) {
    if (waitFor(dom.window.document)) return { dom, errors };
    await new Promise((r) => setTimeout(r, 100));
  }
  return { dom, errors, timedOut: true };
}

let failed = 0;
function check(name, ok, detail = '') {
  console.log((ok ? 'PASS ' : 'FAIL ') + name + (detail ? '  ' + detail : ''));
  if (!ok) failed++;
}

(async () => {
  const q = encodeURIComponent;
  const cases = [
    ['freq', '#stats=freq&top=5', (d) => d.querySelectorAll('#st-table tbody tr').length === 5],
    ['dist by cave', '#stats=dist&word=' + q('יחד') + '&by=cave', (d) => d.querySelectorAll('#st-table tbody tr').length === 4],
    ['colloc', '#stats=colloc&word=' + q('יחד') + '&top=5', (d) => d.querySelectorAll('#st-table tbody tr').length === 5],
    ['keyness', '#stats=keyness&target=1QS&top=5', (d) => d.querySelectorAll('#st-table tbody tr').length === 5],
  ];
  for (const [name, hash, ready] of cases) {
    const { dom, errors, timedOut } = await open(hash, ready);
    const d = dom.window.document;
    const rows = [...d.querySelectorAll('#st-table tbody tr')].map((tr) => [...tr.children].map((c) => c.textContent.trim()));
    check(`stats tab: ${name}`, !timedOut && rows.length > 0, rows[0] ? JSON.stringify(rows[0]) : 'no rows: ' + (d.getElementById('st-status')?.textContent || ''));
    check(`   no page errors (${name})`, errors.length === 0, errors.slice(0, 2).join(' | '));
    check(`   notice hidden (${name})`, d.getElementById('st-notice').hidden === true);
    if (name === 'colloc') {
      check('   summary text shown', /occurs 193 times/.test(d.getElementById('st-meta').textContent), d.getElementById('st-meta').textContent.slice(0, 90));
    }
    dom.window.close();
  }
  // a typo gets suggestions (the page shows them as buttons)
  {
    const { dom, timedOut } = await open('#stats=dist&word=' + q('ירושל'), (d) => d.querySelectorAll('#st-status .suggestions button').length > 0);
    const n = dom.window.document.querySelectorAll('#st-status .suggestions button').length;
    check('suggestions appear for an unknown word', !timedOut && n > 0, n + ' suggestions');
    dom.window.close();
  }
  // search tab still works (data loaded)
  {
    const { dom, timedOut } = await open('', (d) => /words/i.test(d.getElementById('status').textContent) && !/Loading/.test(d.getElementById('status').textContent));
    check('data loads on the Search tab', !timedOut, dom.window.document.getElementById('status').textContent.slice(0, 100));
    dom.window.close();
  }
  process.exit(failed ? 1 : 0);
})();

// Checks how the page saves files. In the program's own window it hands the CSV text to the
// program (window.ipc), keeping the byte-order mark so that Excel reads the Hebrew; in an
// ordinary browser it still starts the normal download.
//
//   npm install            (once, in this folder)
//   dss-search serve --port 8199 &
//   node save.check.js 8199

const { JSDOM, VirtualConsole } = require('jsdom');
const port = process.argv[2];
const base = `http://127.0.0.1:${port}/`;
let failed = 0;
const check = (name, ok, detail = '') => { console.log((ok ? 'PASS ' : 'FAIL ') + name + (detail ? '  ' + detail : '')); if (!ok) failed++; };
const wait = async (cond, ms = 15000) => { const t = Date.now(); while (Date.now() - t < ms) { if (cond()) return true; await new Promise(r => setTimeout(r, 100)); } return false; };

async function open(hash, withProgram) {
  const messages = [], errors = [], downloads = [];
  const vc = new VirtualConsole(); vc.on('jsdomError', e => errors.push(String(e.detail || e.message)));
  const dom = await JSDOM.fromURL(base + hash, { runScripts: 'dangerously', resources: 'usable', pretendToBeVisual: true, virtualConsole: vc,
    beforeParse(w) {
      w.fetch = (u, o) => fetch(new URL(u, w.location.href), o);
      w.TextDecoder = TextDecoder;
      if (withProgram) w.ipc = { postMessage: (m) => messages.push(JSON.parse(m)) };
      // a browser would start a download when a link with a download attribute is clicked
      w.HTMLAnchorElement.prototype.click = function () { downloads.push(this.download || this.getAttribute('download')); };
      w.URL.createObjectURL = () => 'blob:fake'; w.URL.revokeObjectURL = () => {};
    } });
  return { dom, messages, errors, downloads, d: dom.window.document };
}

(async () => {
  // 1. Statistics CSV through the program
  {
    const { dom, messages, errors, d } = await open('#stats=freq&top=3', true);
    await wait(() => d.querySelectorAll('#st-table tbody tr').length === 3);
    d.getElementById('st-csv').dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true, cancelable: true }));
    await wait(() => messages.length > 0);
    const m = messages[0] || {};
    check('stats CSV is handed to the program', m.type === 'save' && m.filename === 'dss-freq.csv', JSON.stringify({ type: m.type, filename: m.filename }));
    check('   it keeps the byte-order mark (Excel needs it)', (m.text || '').charCodeAt(0) === 0xFEFF);
    check('   it has the header and every row, not only the 3 shown', (m.text || '').replace(/^\uFEFF/, '').trim().split('\n').length > 1000, (m.text || '').trim().split('\n').length + ' lines, header: ' + (m.text || '').slice(1, 40));
    check('   Hebrew is intact', /כל/.test(m.text || ''));
    check('   no page errors', errors.length === 0, errors.join(' | '));
    dom.window.close();
  }
  // 2. Search tab CSV through the program
  {
    const { dom, messages, d, errors } = await open('', true);
    await wait(() => /Ready/.test(d.getElementById('status').textContent));
    d.getElementById('q').value = 'יחד';
    d.getElementById('search-form') ? d.getElementById('search-form').dispatchEvent(new dom.window.Event('submit', { cancelable: true })) : d.querySelector('button[type=submit]').click();
    await wait(() => /Found|hits|matches|occurrence/i.test(d.getElementById('status').textContent) || d.querySelectorAll('#results > li').length > 0, 20000);
    const csvButton = d.getElementById('csv');
    csvButton.click();
    await wait(() => messages.length > 0, 5000);
    const m = messages[0] || {};
    check('search CSV is handed to the program', m.type === 'save' && m.filename === 'dss-search.csv', JSON.stringify({ type: m.type, filename: m.filename }));
    check('   it keeps the byte-order mark', (m.text || '').charCodeAt(0) === 0xFEFF);
    check('   it contains the hits', (m.text || '').split('\n').length > 50, (m.text || '').split('\n').length + ' lines');
    dom.window.close();
  }
  // 3. In an ordinary browser (no channel to a program) the normal download still happens
  {
    const { dom, messages, downloads, d } = await open('', false);
    await wait(() => /Ready/.test(d.getElementById('status').textContent));
    d.getElementById('q').value = 'יחד';
    d.getElementById('search-form') ? d.getElementById('search-form').dispatchEvent(new dom.window.Event('submit', { cancelable: true })) : d.querySelector('button[type=submit]').click();
    await wait(() => d.querySelectorAll('#results > li').length > 0 || /Found|hits|matches/i.test(d.getElementById('status').textContent), 20000);
    d.getElementById('csv').click();
    await wait(() => downloads.length > 0, 3000);
    check('browser: normal download used, nothing sent to a program', downloads[0] === 'dss-search.csv' && messages.length === 0, JSON.stringify(downloads));
    dom.window.close();
  }
  process.exit(failed ? 1 : 0);
})();

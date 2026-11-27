import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import test from 'node:test';

function read(path) {
  return readFileSync(new URL(path, import.meta.url), 'utf8');
}

test('offline preview shows the measured replies and note requests', () => {
  const html = read('../docs/index.html');
  const script = html.match(/<script id="replay-data" type="application\/json">([\s\S]*?)<\/script>/);
  assert.ok(script, 'missing offline replay data');
  const replay = JSON.parse(script[1]);
  const cases = JSON.parse(read('../bench/first-use/cases.json')).map(task => task.id);

  for (const harness of ['claude', 'codex']) {
    for (const arm of ['base', 'live', 'static']) {
      const table = read(`../bench/first-use/${harness}-${arm}.tsv`).trimEnd().split('\n');
      const columns = table.shift().split('\t');
      const expected = {};
      for (const line of table) {
        const values = line.split('\t');
        const row = Object.fromEntries(columns.map((column, index) => [column, values[index]]));
        assert.ok(['pass', 'wrong'].includes(row.grade), 'infrastructure failure cannot be displayed as a reply');
        assert.ok(['0', '1'].includes(row.docs));
        expected[row.case] = { answer: row.answer, read: row.docs === '1' };
      }
      assert.deepEqual(Object.keys(expected), cases);
      assert.deepEqual(replay[harness][arm], expected);
    }
  }
});

test('offline print downloads match the verified PDFs', () => {
  const html = read('../docs/index.html');
  const script = html.match(/<script id="print-data" type="application\/json">([\s\S]*?)<\/script>/);
  assert.ok(script, 'missing embedded sheets');
  const files = JSON.parse(script[1]);
  const { download_sha256: hashes } = JSON.parse(read('../bench/relay/results/runs.json'));
  assert.deepEqual(Object.keys(files).sort(), ['changed-brief', 'new-event']);
  for (const [task, file] of Object.entries(files)) {
    const pdf = Buffer.from(file.pdf, 'base64');
    assert.equal(pdf.subarray(0, 5).toString(), '%PDF-');
    assert.equal(createHash('sha256').update(pdf).digest('hex'), hashes[task]);
    assert.equal(Buffer.from(file.png, 'base64').subarray(1, 4).toString(), 'PNG');
  }
});

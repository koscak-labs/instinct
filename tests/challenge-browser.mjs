import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import test from 'node:test';

// Run inside a copy of the offline demo. Only the test copy receives this script.
function exerciseChallenge() {
  const get = id => document.getElementById(id);
  const expect = (ok, why) => { if (!ok) throw new Error(why); };
  const result = document.createElement('output');
  document.body.append(result);
  try {
    expect(get('challenge-dialog').open, 'direct link should open the test');
    let copied;
    Object.defineProperty(navigator, 'clipboard', { value: { writeText: text => { copied = text; return Promise.resolve(); } } });
    for (const slot of [0, 1]) {
      Object.defineProperty(crypto, 'getRandomValues', { configurable: true, value: bytes => bytes.fill(slot) });
      for (const choice of ['0', '1', 'both', 'neither']) {
        get('challenge-setup').requestSubmit();
        expect(!get('challenge-setup').hidden, 'empty criteria should be rejected');
        get('challenge-criterion').value = 'Private criterion';
        get('challenge-setup').requestSubmit();
        expect(get('challenge-setup').hidden && !get('challenge-paste').hidden, 'freeze criterion');
        get('challenge-empty').value = '<b>Private baseline</b>';
        get('challenge-taught').value = 'Private learned answer';
        get('challenge-paste').requestSubmit();
        expect(!get('challenge-judge').hidden && get('challenge-result').hidden, 'answers before labels');
        const baseline = get(slot === 0 ? 'answer-b' : 'answer-a');
        expect(baseline.textContent === '<b>Private baseline</b>' && !baseline.children.length, 'paste as inert text');
        document.querySelector('[data-pick="' + choice + '"]').click();
        const wanted = { both: 'Both answers met my test.', neither: 'Neither answer met my test.' }[choice]
          || (Number(choice) === slot ? 'I picked the answer with the lesson.' : 'I picked the answer without the lesson.');
        expect(get('challenge-verdict').textContent === wanted, 'verdict should follow vote');
        expect(get('challenge-key').textContent.startsWith(slot === 0 ? 'A had' : 'B had'), 'reveal should match real order');
        get('copy-verdict').click();
        expect(copied.endsWith(wanted) && !copied.includes('Private'), 'copy only the verdict');
        get('restart-challenge').click();
        expect(!get('challenge-criterion').value && !get('challenge-taught').value && !get('answer-a').textContent, 'clear prior task');
      }
    }
    result.id = 'verified-challenge';
  } catch (err) {
    result.id = 'failed-challenge';
    result.textContent = String(err);
  }
}

test('the offline challenge hides labels, scores every vote and copies only the verdict', () => {
  const dir = mkdtempSync(join(tmpdir(), 'instinct-browser-'));
  try {
    const html = readFileSync(new URL('../docs/index.html', import.meta.url), 'utf8');
    const fixture = join(dir, 'test.html');
    writeFileSync(fixture, html.replace('</body>', '<script>(' + exerciseChallenge.toString() + ')()</script></body>'));
    const output = execFileSync(process.env.CHROMIUM || 'google-chrome', [
      '--headless', '--disable-gpu', '--disable-background-networking', '--disable-dev-shm-usage',
      '--user-data-dir=' + join(dir, 'profile'), '--dump-dom', pathToFileURL(fixture).href + '#challenge',
    ], { encoding: 'utf8', timeout: 30000, maxBuffer: 2 * 1024 * 1024, stdio: ['ignore', 'pipe', 'pipe'] });
    assert.ok(output.includes('<output id="verified-challenge">'), output.match(/<output id="failed-challenge">([^<]*)/)?.[1] || 'browser test did not finish');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

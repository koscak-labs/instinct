import assert from 'node:assert/strict';
import { execFile, execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { promisify } from 'node:util';
import test from 'node:test';

const executable = process.env.INSTINCT_TEST_BIN;
const env = Object.fromEntries(Object.entries(process.env).filter(([k]) => !k.startsWith('INSTINCT_')));
function fixture(t) {
  assert.ok(executable, 'set INSTINCT_TEST_BIN to the built executable');
  const dir = mkdtempSync(join(tmpdir(), 'instinct-native-test-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const native = join(dir, 'bank.hxdb');
  const legacy = join(dir, 'bank.jsonl');
  const options = { env: { ...env, INSTINCT_PROJECT: 'studio' }, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] };
  function run(path, ...args) {
    return execFileSync(executable, [...args, '--store', path], options);
  }
  function decide(path, q, project='studio') {
    return JSON.parse(run(path, 'decide', '--q', q, '--project', project)).experiences;
  }
  const discovery = { kind: 'discovery', id: 'old', title: 'Approved text', when: 'approved headline repair',
    how: 'Keep approved copy', found: 'Reader accepted edit', evidence: 'Text with a tab\tand a line\nbreak ✓',
    source: 'banked', state: 'candidate', scope_project: 'studio' };
  return { dir, native, legacy, options, discovery, run, decide };
}

test('native migration preserves history, refuses replacement and rejects torn input', t => {
  const {dir, native, legacy, discovery, run, decide} = fixture(t);
  const events = [discovery, {kind:'hit', id:'old', outcome_id:'check-1'}, {kind:'miss', id:'old'},
    {kind:'retract', of:'miss', id:'old'}, {kind:'review', id:'old', state:'quarantined', reason:'changed brief'}];
  const original = events.map(v => JSON.stringify(v)).join('\n');
  writeFileSync(legacy, original);
  run(legacy, 'migrate', '--to', native);
  assert.equal(readFileSync(legacy, 'utf8'), original);
  assert.equal(run(native,'export'), original+'\n');
  assert.deepEqual(decide(native, 'approved headline repair'), []);
  assert.match(readFileSync(native, 'utf8'), /^event:0{16}\tV\t[a-f0-9]{16}:/);
  const bytes = readFileSync(native);
  assert.throws(() => run(legacy, 'migrate', '--to', native));
  assert.deepEqual(bytes, readFileSync(native));
  writeFileSync(legacy, original+'\n{"kind":');
  const fresh = join(dir, 'new.hxdb');
  assert.throws(() => run(legacy, 'migrate', '--to', fresh));
  assert.equal(existsSync(fresh), false);
  writeFileSync(native, bytes.toString().replace('changed brief', 'altered brief'));
  assert.throws(() => decide(native,'approved headline repair'), /invalid Helix DB event/);
  writeFileSync(native, bytes.subarray(0,bytes.length-4));
  assert.throws(() => run(native, 'hit', 'old', '--outcome-id', 'x'));
});

test('native earned cue can tolerate a typo, abstain on ambiguity and survive concurrent feedback', async t => {
  const {native, legacy, options, discovery, run, decide} = fixture(t);
  writeFileSync(legacy, JSON.stringify(discovery));
  run(legacy,'migrate','--to',native);
  const typo = 'aproved hedline repair';
  assert.deepEqual(decide(native,typo), []);
  await Promise.all(Array.from({length:12}, () => promisify(execFile)(executable,
    ['hit','old','--outcome-id','shared-check','--store',native], options)));
  assert.equal(decide(native,typo)[0]?.hits, 1);
  assert.deepEqual(decide(native,typo,'another-project'), []);
  assert.deepEqual(decide(native,'cancel aproved hedline repair'), []);
  assert.deepEqual(decide(native,'parking permit renewal'), []);
  const rows = run(native,'export').trim().split('\n').map(line => JSON.parse(line));
  assert.equal(rows.filter(row => row.kind === 'hit').length, 1);
  run(native,'bank','--title','Second outcome','--when',discovery.when,'--found','Another result',
    '--how','Use the desk copy','--evidence','Editor checked');
  const second = run(native,'export').trim().split('\n').map(line => JSON.parse(line)).at(-1);
  run(native,'hit',second.id,'--outcome-id','checked-2');
  assert.deepEqual(decide(native,typo), []);
  run(native,'review',second.id,'--state','quarantined','--reason','retired','--source-ref','check');
  assert.equal(decide(native,typo)[0]?.id, 'old');
  run(native,'review','old','--state','quarantined','--reason','changed','--source-ref','check');
  assert.deepEqual(decide(native,typo), []);
});

test('one native MCP process sees external feedback, changed scope, expiry and disk damage', async t => {
  const {native, legacy, discovery, run} = fixture(t);
  const {spawn} = await import('node:child_process');
  const {createInterface} = await import('node:readline');
  const until = Math.floor(Date.now()/1000) + 2;
  const second = {...discovery,id:'other',scope_project:'gallery',how:'Gallery rule'};
  writeFileSync(legacy, [discovery,second,{...discovery,id:'expiring',when:'sleeve pickup',valid_until:until}].map(v=>JSON.stringify(v)).join('\n'));
  run(legacy,'migrate','--to',native);
  const child = spawn(executable,['mcp'],{env:{...env,INSTINCT_STORE:native},stdio:['pipe','pipe','pipe']});
  t.after(() => { child.stdin.end(); child.kill(); });
  const input = createInterface({input:child.stdout})[Symbol.asyncIterator]();
  async function recall(project,q) {
    child.stdin.write(JSON.stringify({jsonrpc:'2.0',id:1,method:'tools/call',params:{name:'instinct_recall',arguments:{project,q}}})+'\n');
    return JSON.parse((await input.next()).value).result;
  }
  assert.match((await recall('studio', 'approved headline repair')).content[0].text,/\[old\]/);
  run(native,'hit','old','--outcome-id','proof');
  assert.match((await recall('studio', 'aproved hedline repair')).content[0].text,/\[old\]/);
  assert.match((await recall('gallery','approved headline repair')).content[0].text,/\[other\]/);
  assert.doesNotMatch((await recall('gallery','approved headline repair')).content[0].text,/\[old\]/);
  assert.match((await recall('studio','sleeve pickup')).content[0].text,/\[expiring\]/);
  await new Promise(resolve=>setTimeout(resolve,Math.max(0,(until+1)*1000+20-Date.now())));
  assert.match((await recall('studio','sleeve pickup')).content[0].text,/nothing banked matches/);
  run(native,'review','old','--state','quarantined','--reason','superseded','--source-ref','desk-check');
  assert.match((await recall('studio','approved headline repair')).content[0].text,/nothing banked matches/);
  const previous = readFileSync(native,'utf8');
  writeFileSync(native,previous.replace('superseded','overturned'));
  assert.equal((await recall('studio','approved headline repair')).isError,true);
});

// Keep the public runtime editable when refreshing the Instinct adapter with hxc.
import { readFileSync, writeFileSync } from 'node:fs';
const path = process.argv[2];
if (!path) throw Error('supply the formatted Rust export of src/helix_db.hlx');
const source = readFileSync(path, 'utf8');
const start = source.indexOf('// hlx:2\n') + '// hlx:2\n'.length;
const end = source.indexOf('// hlx:3\n', start);
if (end <= start || !source.slice(start, end).includes('fn db_putv('))
  throw Error('expected the GGGGT database block on line 2 of src/helix_db.hlx');
if (process.argv[3]) writeFileSync(process.argv[3], source.slice(start, end));
writeFileSync(path, source.slice(0, start) + 'include!("../runtime/helix-db.rs");\n' + source.slice(end));

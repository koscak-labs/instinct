// ============================================================================
// Helix embedded store  (codon GGGGT / db)  -- pure-std, 0 crates, single-file.
// Append-only log (Bitcask-style): each record is one line  key <TAB> op <TAB> value
// (op P=put, D=delete tombstone); last write wins. db_load replays -> live records.
// Durable append; db_compact rewrites to live records only (reclaims overwrite/tombstone space).
//   db_put(file,key,val) / db_get(file,key) / db_del(file,key) / db_has(file,key)
//   db_keys(file) / db_all(file) / db_count(file)
//   db_find(file,needle)      -- values containing a substring
//   db_field(file,field,val)  -- JSON records whose top-level "field":"val" matches
//   db_compact(file)          -- reclaim space
// ============================================================================
fn db_esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}
fn db_unesc(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('t') => out.push('\t'),
                Some('n') => out.push('\n'),
                Some('\\') => out.push('\\'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}
fn db_load(file: &str) -> std::collections::BTreeMap<String, String> {
    let mut map = std::collections::BTreeMap::new();
    let body = std::fs::read_to_string(file).unwrap_or_default();
    for line in body.lines() {
        if line.is_empty() {
            continue;
        }
        let mut it = line.splitn(3, '\t');
        let k = it.next().unwrap_or("");
        let op = it.next().unwrap_or("");
        let v = it.next().unwrap_or("");
        if k.is_empty() {
            continue;
        }
        let key = db_unesc(k);
        if op == "D" {
            map.remove(&key);
        } else if op == "V" {
            let val = v.splitn(2, ':').nth(1).unwrap_or("");
            map.insert(key, db_unesc(val));
        } else {
            map.insert(key, db_unesc(v));
        }
    }
    map
}
fn db_put(file: &str, key: &str, value: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)
    {
        let _ = writeln!(f, "{}\tP\t{}", db_esc(key), db_esc(value));
    }
}
fn db_del(file: &str, key: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)
    {
        let _ = writeln!(f, "{}\tD\t", db_esc(key));
    }
}
fn db_get(file: &str, key: &str) -> String {
    db_load(file).get(key).cloned().unwrap_or_default()
}
fn db_has(file: &str, key: &str) -> bool {
    db_load(file).contains_key(key)
}
fn db_keys(file: &str) -> Vec<String> {
    db_load(file).keys().cloned().collect()
}
fn db_all(file: &str) -> Vec<String> {
    db_load(file).values().cloned().collect()
}
fn db_count(file: &str) -> usize {
    db_load(file).len()
}
fn db_find(file: &str, needle: &str) -> Vec<String> {
    db_load(file)
        .into_iter()
        .map(|(_, v)| v)
        .filter(|v| v.contains(needle))
        .collect()
}
fn db_field(file: &str, field: &str, val: &str) -> Vec<String> {
    let pat = format!("\"{}\":\"{}\"", field, val);
    db_load(file)
        .into_iter()
        .map(|(_, v)| v)
        .filter(|v| v.contains(&pat))
        .collect()
}
fn db_compact(file: &str) {
    let map = db_load(file);
    let mut body = String::new();
    for (k, v) in &map {
        body.push_str(&format!("{}\tP\t{}\n", db_esc(k), db_esc(v)));
    }
    let tmp = format!("{}.tmp", file);
    if std::fs::write(&tmp, body).is_ok() {
        let _ = std::fs::rename(&tmp, file);
    }
}
// ---- DNA-grade verified records (1.9.0): per-record FNV-1a checksum (DNA carries redundancy +
//      proofreading; this is that for storage). db_putv stores the value WITH its checksum; db_getv
//      recomputes on read and, if the latest record is corrupt, HEALS by falling back to the most
//      recent prior version that still verifies (the append-log's own history is the redundancy),
//      and ABSTAINS ("") if nothing verifies -- never serves a known-bad row. db_verify scans the
//      store and reports OK vs corrupt keys. db_load reads V records transparently (clean value).
fn db_fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}
fn db_putv(file: &str, key: &str, value: &str) {
    use std::io::Write;
    let stored = format!("{:016x}:{}", db_fnv(value), db_esc(value));
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)
    {
        let _ = writeln!(f, "{}\tV\t{}", db_esc(key), stored);
    }
}
fn db_getv(file: &str, key: &str) -> String {
    let body = std::fs::read_to_string(file).unwrap_or_default();
    let ekey = db_esc(key);
    let mut good: Option<String> = None;
    for line in body.lines() {
        if line.is_empty() {
            continue;
        }
        let mut it = line.splitn(3, '\t');
        let k = it.next().unwrap_or("");
        let op = it.next().unwrap_or("");
        let v = it.next().unwrap_or("");
        if k != ekey {
            continue;
        }
        if op == "D" {
            good = None;
        } else if op == "V" {
            let mut p = v.splitn(2, ':');
            let hex = p.next().unwrap_or("");
            let val = db_unesc(p.next().unwrap_or(""));
            if format!("{:016x}", db_fnv(&val)) == hex {
                good = Some(val);
            }
        } else {
            good = Some(db_unesc(v));
        }
    }
    good.unwrap_or_default()
}
fn db_verify(file: &str) -> String {
    let body = std::fs::read_to_string(file).unwrap_or_default();
    let mut latest: std::collections::BTreeMap<String, (String, String)> =
        std::collections::BTreeMap::new();
    for line in body.lines() {
        if line.is_empty() {
            continue;
        }
        let mut it = line.splitn(3, '\t');
        let k = it.next().unwrap_or("");
        let op = it.next().unwrap_or("");
        let v = it.next().unwrap_or("");
        if k.is_empty() {
            continue;
        }
        if op == "D" {
            latest.remove(k);
        } else {
            latest.insert(k.to_string(), (op.to_string(), v.to_string()));
        }
    }
    let mut ok = 0usize;
    let mut corrupt: Vec<String> = Vec::new();
    for (k, (op, v)) in &latest {
        if op == "V" {
            let mut p = v.splitn(2, ':');
            let hex = p.next().unwrap_or("");
            let raw = p.next().unwrap_or("");
            if format!("{:016x}", db_fnv(&db_unesc(raw))) == hex {
                ok += 1;
            } else {
                corrupt.push(db_unesc(k));
            }
        } else {
            ok += 1;
        }
    }
    format!(
        "ok={} corrupt={} corrupt_keys={:?}",
        ok,
        corrupt.len(),
        corrupt
    )
}
// ---- Integrity guard for FOREIGN JSONL append-logs (engram/conv-mem/kronika etc): db_verify needs
//      our checksum format, so for a plain JSONL store we check STRUCTURAL well-formedness per line
//      (balanced braces/brackets outside strings, balanced quotes) -- catches the real silent
//      corruption mode for append-logs: a truncated/partial last line or a garbled record. Streams
//      line-by-line (safe on 100MB+ files). Detection only (cannot heal a foreign format).
fn db_json_wellformed(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return true;
    }
    let b0 = t.as_bytes()[0];
    if b0 != b'{' && b0 != b'[' {
        return false;
    }
    let mut depth: i64 = 0;
    let mut in_str = false;
    let mut esc = false;
    let mut opened = false;
    for c in t.chars() {
        if in_str {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
        } else {
            match c {
                '"' => in_str = true,
                '{' | '[' => {
                    depth += 1;
                    opened = true;
                }
                '}' | ']' => {
                    depth -= 1;
                    if depth < 0 {
                        return false;
                    }
                }
                _ => {}
            }
        }
    }
    depth == 0 && !in_str && opened
}
fn db_jsonl_check(file: &str) -> String {
    use std::io::{BufRead, BufReader};
    let f = match std::fs::File::open(file) {
        Ok(f) => f,
        Err(_) => return format!("file={} error=cannot_open", file),
    };
    let r = BufReader::new(f);
    let mut total = 0usize;
    let mut ok = 0usize;
    let mut bad: Vec<usize> = Vec::new();
    for (i, line) in r.lines().enumerate() {
        let line = match line {
            Ok(l) => l,
            Err(_) => {
                total += 1;
                if bad.len() < 10 {
                    bad.push(i + 1);
                }
                continue;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        total += 1;
        if db_json_wellformed(&line) {
            ok += 1;
        } else if bad.len() < 10 {
            bad.push(i + 1);
        }
    }
    format!(
        "file={} lines={} ok={} corrupt={} first_bad_lines={:?}",
        file,
        total,
        ok,
        total - ok,
        bad
    )
}
// ---- DNA 2-bit packing (1.11.0): DNA stores 2 bits per base (A/C/G/T). For ACGT/genomic data,
//      db_pack_file packs 4 bases into 1 byte -> ~4x smaller than the ACGT-text file (the real
//      density edge of DNA, applied to sequence storage). Lossless on the sequence (case-folded to
//      upper, non-ACGT chars dropped). NOT a general compressor -- only for the 4-symbol alphabet.
//      8-byte LE header = base count, so db_unpack_file restores the exact length.
fn db_pack_file(seq_file: &str, out_file: &str) -> usize {
    let s = std::fs::read(seq_file).unwrap_or_default();
    let mut bytes: Vec<u8> = Vec::with_capacity(s.len() / 4 + 8);
    let mut cur: u8 = 0;
    let mut n: u8 = 0;
    let mut count: usize = 0;
    for &c in &s {
        let code = match c {
            b'A' | b'a' => 0u8,
            b'C' | b'c' => 1,
            b'G' | b'g' => 2,
            b'T' | b't' => 3,
            _ => continue,
        };
        cur = (cur << 2) | code;
        n += 1;
        count += 1;
        if n == 4 {
            bytes.push(cur);
            cur = 0;
            n = 0;
        }
    }
    if n > 0 {
        cur <<= 2 * (4 - n);
        bytes.push(cur);
    }
    let mut out = (count as u64).to_le_bytes().to_vec();
    out.extend_from_slice(&bytes);
    let _ = std::fs::write(out_file, &out);
    count
}
fn db_unpack_file(packed_file: &str, out_file: &str) -> usize {
    let data = std::fs::read(packed_file).unwrap_or_default();
    if data.len() < 8 {
        let _ = std::fs::write(out_file, "");
        return 0;
    }
    let mut hdr = [0u8; 8];
    hdr.copy_from_slice(&data[0..8]);
    let count = u64::from_le_bytes(hdr) as usize;
    let tbl = [b'A', b'C', b'G', b'T'];
    let mut out: Vec<u8> = Vec::with_capacity(count);
    for &b in &data[8..] {
        for shift in [6i32, 4, 2, 0] {
            if out.len() >= count {
                break;
            }
            out.push(tbl[((b >> shift) & 0x3) as usize]);
        }
        if out.len() >= count {
            break;
        }
    }
    let _ = std::fs::write(out_file, &out);
    out.len()
}
// ---- Associative recall (1.12.0): content-addressable FUZZY lookup -- find records by similarity
//      to a query, not by exact key (the recall-by-pattern that key-lookup DBs cannot do; the
//      Hopfield/associative-memory idea at the lexical level). Similarity = character-trigram
//      Jaccard (typo/partial tolerant), pure-std. db_recall returns the top-k values ranked
//      "score<TAB>value". Honest: this is LEXICAL similarity, not semantic embeddings.
fn db_trigrams(s: &str) -> std::collections::BTreeSet<String> {
    let chars: Vec<char> = s.to_lowercase().chars().collect();
    let mut set = std::collections::BTreeSet::new();
    if chars.len() < 3 {
        if !chars.is_empty() {
            set.insert(chars.iter().collect::<String>());
        }
        return set;
    }
    for w in chars.windows(3) {
        set.insert(w.iter().collect::<String>());
    }
    set
}
fn db_sim(a: &std::collections::BTreeSet<String>, b: &std::collections::BTreeSet<String>) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let inter = a.intersection(b).count();
    let uni = a.len() + b.len() - inter;
    if uni == 0 {
        0.0
    } else {
        inter as f64 / uni as f64
    }
}
fn db_recall(file: &str, query: &str, topk: usize) -> Vec<String> {
    let q = db_trigrams(query);
    let mut scored: Vec<(f64, String)> = db_load(file)
        .into_iter()
        .map(|(_, v)| {
            let s = db_sim(&q, &db_trigrams(&v));
            (s, v)
        })
        .filter(|(s, _)| *s > 0.0)
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored
        .into_iter()
        .take(topk.max(1))
        .map(|(s, v)| format!("{:.3}\t{}", s, v))
        .collect()
}
// ---- Predictive / derived store (1.13.0): the KODON ethos as storage -- for an integer sequence
//      that follows an exact law, store the GENERATOR not the data (a few ints), and regenerate on
//      read. Bounded law library: order-2 linear recurrence (Fibonacci-like) + finite-difference
//      polynomial (constant/arith/quadratic/cubic...). EXACT-OR-ABSTAIN: a law is stored only if it
//      decodes back to the input byte-for-byte; otherwise the sequence is stored RAW (no loss, no
//      false compression). db_put_seq picks the law, db_get_seq regenerates, db_seq_law reports it.
fn db_seq_parse(csv: &str) -> Vec<i64> {
    csv.split(',')
        .filter_map(|x| x.trim().parse::<i64>().ok())
        .collect()
}
fn db_seq_lr2(v: &[i64]) -> Option<(i64, i64)> {
    if v.len() < 4 {
        return None;
    }
    let (a0, a1, a2, a3) = (v[0], v[1], v[2], v[3]);
    let det = a1 * a1 - a2 * a0;
    if det == 0 {
        return None;
    }
    let pnum = a2 * a1 - a3 * a0;
    let qnum = a1 * a3 - a2 * a2;
    if pnum % det != 0 || qnum % det != 0 {
        return None;
    }
    let (p, q) = (pnum / det, qnum / det);
    for i in 2..v.len() {
        if v[i] != p * v[i - 1] + q * v[i - 2] {
            return None;
        }
    }
    Some((p, q))
}
fn db_seq_fd(v: &[i64], maxdeg: usize) -> Option<Vec<i64>> {
    if v.len() < 2 {
        return None;
    }
    let mut cur = v.to_vec();
    let mut seeds = vec![cur[0]];
    let lim = maxdeg.min(v.len() - 1);
    for _ in 0..lim {
        if cur.iter().all(|&x| x == cur[0]) {
            return Some(seeds);
        }
        let mut nxt = Vec::with_capacity(cur.len() - 1);
        for i in 1..cur.len() {
            nxt.push(cur[i] - cur[i - 1]);
        }
        cur = nxt;
        seeds.push(cur[0]);
    }
    if cur.iter().all(|&x| x == cur[0]) {
        Some(seeds)
    } else {
        None
    }
}
fn db_seq_fd_gen(seeds: &[i64], n: usize) -> Vec<i64> {
    if seeds.is_empty() || n == 0 {
        return Vec::new();
    }
    let k = seeds.len() - 1;
    let base_len = if n > k { n - k } else { 1 };
    let mut level: Vec<i64> = vec![seeds[k]; base_len];
    for j in (0..k).rev() {
        let mut up = Vec::with_capacity(level.len() + 1);
        let mut acc = seeds[j];
        up.push(acc);
        for &d in &level {
            acc += d;
            up.push(acc);
        }
        level = up;
    }
    level.into_iter().take(n).collect()
}
fn db_seq_decode(enc: &str) -> Vec<i64> {
    if let Some(r) = enc.strip_prefix("RAW:") {
        return db_seq_parse(r);
    }
    if let Some(r) = enc.strip_prefix("LR:") {
        let mut it = r.splitn(2, ':');
        let n: usize = it.next().unwrap_or("0").parse().unwrap_or(0);
        let p: Vec<i64> = it
            .next()
            .unwrap_or("")
            .split(',')
            .filter_map(|x| x.parse().ok())
            .collect();
        if p.len() != 4 {
            return Vec::new();
        }
        let mut out: Vec<i64> = Vec::with_capacity(n);
        for i in 0..n {
            let x = if i == 0 {
                p[0]
            } else if i == 1 {
                p[1]
            } else {
                p[2] * out[i - 1] + p[3] * out[i - 2]
            };
            out.push(x);
        }
        return out;
    }
    if let Some(r) = enc.strip_prefix("FD:") {
        let mut it = r.splitn(2, ':');
        let n: usize = it.next().unwrap_or("0").parse().unwrap_or(0);
        let seeds: Vec<i64> = it
            .next()
            .unwrap_or("")
            .split(',')
            .filter_map(|x| x.parse().ok())
            .collect();
        return db_seq_fd_gen(&seeds, n);
    }
    Vec::new()
}
fn db_put_seq(file: &str, key: &str, csv: &str) -> String {
    let v = db_seq_parse(csv);
    let n = v.len();
    // MIN-TERMS GUARD (1.13.1): a fit needs enough terms to be a LAW, not an overfit. LR2 is derived
    // from 4 terms -> require >=6 (>=2 verification terms beyond). An FD degree-d poly uses d+1 seeds
    // and ANY d+1 points fit a degree-d poly exactly -> require n >= 2*(d+1). Else fall back to RAW.
    if n >= 6 {
        if let Some((p, q)) = db_seq_lr2(&v) {
            let enc = format!("LR:{}:{},{},{},{}", n, v[0], v[1], p, q);
            if db_seq_decode(&enc) == v {
                db_put(file, key, &enc);
                return format!("LR p={} q={} ({} terms -> 4 ints)", p, q, n);
            }
        }
    }
    if let Some(seeds) = db_seq_fd(&v, 8) {
        if n >= 2 * seeds.len() {
            let enc = format!(
                "FD:{}:{}",
                n,
                seeds
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            );
            if db_seq_decode(&enc) == v {
                db_put(file, key, &enc);
                return format!(
                    "FD deg={} ({} terms -> {} ints)",
                    seeds.len() - 1,
                    n,
                    seeds.len()
                );
            }
        }
    }
    db_put(file, key, &format!("RAW:{}", csv));
    format!("raw ({} terms, no exact law)", n)
}
fn db_get_seq(file: &str, key: &str) -> String {
    db_seq_decode(&db_get(file, key))
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join(",")
}
fn db_seq_law(file: &str, key: &str) -> String {
    db_get(file, key)
        .split(':')
        .next()
        .unwrap_or("none")
        .to_string()
}
// ---- SQL power-tool: real SQL with 0 Rust crates. Prefer the sqlite3 CLI (HXC_SQLITE overrides
//      the path); fall back to python's built-in sqlite3 module. Same shell-to-a-tool pattern
//      px_fetch uses for curl. db_sql = statements; db_rows = query (rows, tab-joined columns).
fn db_sqlite_exe() -> String {
    std::env::var("HXC_SQLITE").unwrap_or_else(|_| "sqlite3".to_string())
}
fn db_sql_available() -> bool {
    std::process::Command::new(db_sqlite_exe())
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
        || std::process::Command::new("python")
            .arg("-c")
            .arg("import sqlite3")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
}
fn db_sql(dbfile: &str, sql: &str) -> bool {
    if let Ok(o) = std::process::Command::new(db_sqlite_exe())
        .arg("-batch")
        .arg(dbfile)
        .arg(sql)
        .output()
    {
        if o.status.success() {
            return true;
        }
    }
    let py = "import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); c.executescript(sys.argv[2]); c.commit(); c.close()";
    std::process::Command::new("python")
        .arg("-c")
        .arg(py)
        .arg(dbfile)
        .arg(sql)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
fn db_rows(dbfile: &str, sql: &str) -> Vec<String> {
    if let Ok(o) = std::process::Command::new(db_sqlite_exe())
        .arg("-batch")
        .arg("-noheader")
        .arg("-separator")
        .arg("\t")
        .arg(dbfile)
        .arg(sql)
        .output()
    {
        if o.status.success() {
            return String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.to_string())
                .collect();
        }
    }
    let py = "import sqlite3,sys; print('\\n'.join('\\t'.join('' if x is None else str(x) for x in r) for r in sqlite3.connect(sys.argv[1]).execute(sys.argv[2]).fetchall()))";
    match std::process::Command::new("python")
        .arg("-c")
        .arg(py)
        .arg(dbfile)
        .arg(sql)
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect(),
        _ => Vec::new(),
    }
}
fn db_cell(dbfile: &str, sql: &str) -> String {
    db_rows(dbfile, sql).into_iter().next().unwrap_or_default()
}

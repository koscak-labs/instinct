//! Instinct: situation-triggered recall of discoveries and their methods.
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
mod demo;
#[allow(dead_code)]
mod eligibility;
#[allow(dead_code)]
mod helix_db;
#[allow(dead_code)]
mod journal;
#[allow(dead_code)]
mod recall_index;
use journal::append;

const DEFAULT_MODEL: &str = "nomic-embed-text";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const IO_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_RESPONSE: usize = 16 * 1024 * 1024;
const MAX_EMBED_BYTES: usize = 6000;

// ---------------------------------------------------------------- JSON

#[derive(Debug, Clone, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
    fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            _ => None,
        }
    }
}

/// Escape `s` as the body of a JSON string (no surrounding quotes).
fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) -> Result<(), String> {
        self.ws();
        if self.b.get(self.i) == Some(&c) {
            self.i += 1;
            Ok(())
        } else {
            Err(format!("expected '{}' at byte {}", c as char, self.i))
        }
    }
    fn lit(&mut self, word: &str, v: Json) -> Result<Json, String> {
        if self.b[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(format!("bad literal at byte {}", self.i))
        }
    }
    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        match self.b.get(self.i) {
            None => Err("unexpected end of JSON".into()),
            Some(b'{') => {
                self.i += 1;
                let mut fields = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b'}') {
                    self.i += 1;
                    return Ok(Json::Obj(fields));
                }
                loop {
                    self.ws();
                    let k = self.string()?;
                    self.eat(b':')?;
                    fields.push((k, self.value()?));
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Json::Obj(fields));
                        }
                        _ => return Err(format!("expected ',' or '}}' at byte {}", self.i)),
                    }
                }
            }
            Some(b'[') => {
                self.i += 1;
                let mut items = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b']') {
                    self.i += 1;
                    return Ok(Json::Arr(items));
                }
                loop {
                    items.push(self.value()?);
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(Json::Arr(items));
                        }
                        _ => return Err(format!("expected ',' or ']' at byte {}", self.i)),
                    }
                }
            }
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.lit("true", Json::Bool(true)),
            Some(b'f') => self.lit("false", Json::Bool(false)),
            Some(b'n') => self.lit("null", Json::Null),
            Some(_) => self.number(),
        }
    }
    fn number(&mut self) -> Result<Json, String> {
        let start = self.i;
        while self.i < self.b.len()
            && matches!(
                self.b[self.i],
                b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E'
            )
        {
            self.i += 1;
        }
        std::str::from_utf8(&self.b[start..self.i])
            .ok()
            .and_then(|t| t.parse::<f64>().ok())
            .map(Json::Num)
            .ok_or_else(|| format!("bad number at byte {start}"))
    }
    fn hex4(&mut self) -> Result<u32, String> {
        let h = self
            .b
            .get(self.i..self.i + 4)
            .ok_or("truncated \\u escape")?;
        let v = std::str::from_utf8(h)
            .ok()
            .and_then(|t| u32::from_str_radix(t, 16).ok())
            .ok_or("bad \\u escape")?;
        self.i += 4;
        Ok(v)
    }
    fn string(&mut self) -> Result<String, String> {
        if self.b.get(self.i) != Some(&b'"') {
            return Err(format!("expected string at byte {}", self.i));
        }
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = *self.b.get(self.i).ok_or("unterminated string")?;
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = *self.b.get(self.i).ok_or("unterminated escape")?;
                    self.i += 1;
                    let ch = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'b' => '\u{08}',
                        b'f' => '\u{0c}',
                        b'u' => {
                            let mut cp = self.hex4()?;
                            if (0xD800..0xDC00).contains(&cp)
                                && self.b[self.i..].starts_with(b"\\u")
                            {
                                self.i += 2;
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return Err("bad surrogate pair".into());
                                }
                                cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                            }
                            char::from_u32(cp).ok_or("invalid code point")?
                        }
                        _ => return Err(format!("bad escape '\\{}'", e as char)),
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                _ => out.push(c),
            }
        }
        String::from_utf8(out).map_err(|_| "invalid UTF-8 in string".to_string())
    }
}

fn parse_json(text: &str) -> Result<Json, String> {
    let mut p = Parser {
        b: text.as_bytes(),
        i: 0,
    };
    let v = p.value()?;
    p.ws();
    if p.i != p.b.len() {
        return Err(format!("trailing data at byte {}", p.i));
    }
    Ok(v)
}

fn json_floats(v: &Json) -> Option<Vec<f32>> {
    match v {
        Json::Arr(items) => items.iter().map(|n| n.as_f64().map(|f| f as f32)).collect(),
        _ => None,
    }
}

// ---------------------------------------------------------------- embeddings over raw HTTP

/// Decode an HTTP/1.1 chunked body.
fn dechunk(mut body: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        let nl = body
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or("bad chunk framing")?;
        let size_txt = std::str::from_utf8(&body[..nl]).map_err(|_| "bad chunk size")?;
        let size = usize::from_str_radix(size_txt.split(';').next().unwrap_or("").trim(), 16)
            .map_err(|_| "bad chunk size")?;
        body = &body[nl + 2..];
        if size == 0 {
            return Ok(out);
        }
        if body.len() < size + 2 {
            return Err("truncated chunk".into());
        }
        out.extend_from_slice(&body[..size]);
        body = &body[size + 2..];
    }
}

/// POST `body` as JSON to `path` on `addr`; returns the response body on HTTP 200.
fn http_post_json(addr: &str, path: &str, body: &str) -> Result<String, String> {
    let sock = addr
        .to_socket_addrs()
        .map_err(|e| format!("cannot resolve {addr}: {e}"))?
        .next()
        .ok_or_else(|| format!("no address for {addr}"))?;
    let mut s = TcpStream::connect_timeout(&sock, CONNECT_TIMEOUT)
        .map_err(|e| format!("cannot connect to {addr}: {e}"))?;
    let _ = s.set_read_timeout(Some(IO_TIMEOUT));
    let _ = s.set_write_timeout(Some(IO_TIMEOUT));
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes())
        .map_err(|e| format!("send to {addr} failed: {e}"))?;
    let mut raw = Vec::new();
    s.take(MAX_RESPONSE as u64)
        .read_to_end(&mut raw)
        .map_err(|e| format!("read from {addr} failed: {e}"))?;
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("malformed HTTP response")?;
    let head = String::from_utf8_lossy(&raw[..split]).to_string();
    let status = head.lines().next().unwrap_or("");
    let payload = &raw[split + 4..];
    let chunked = head.lines().any(|l| {
        let l = l.to_ascii_lowercase();
        l.starts_with("transfer-encoding:") && l.contains("chunked")
    });
    let bytes = if chunked {
        dechunk(payload)?
    } else {
        payload.to_vec()
    };
    let text = String::from_utf8(bytes).map_err(|_| "response is not UTF-8".to_string())?;
    if status.split_whitespace().nth(1) != Some("200") {
        return Err(format!(
            "{addr}{path} answered '{status}': {}",
            text.chars().take(200).collect::<String>()
        ));
    }
    Ok(text)
}

/// Pull the single vector out of `{"embeddings":[[f32,...]]}`.
fn parse_embedding(response: &str) -> Result<Vec<f32>, String> {
    let v = parse_json(response)?;
    if let Some(err) = v.get("error").and_then(Json::as_str) {
        return Err(format!("ollama error: {err}"));
    }
    let emb = match v.get("embeddings") {
        Some(Json::Arr(rows)) if rows.len() == 1 => json_floats(&rows[0]),
        _ => None,
    }
    .ok_or("response has no single numeric \"embeddings\" row")?;
    if emb.is_empty() || emb.iter().any(|f| !f.is_finite()) {
        return Err("embedding is empty or non-finite".into());
    }
    Ok(emb)
}

/// Cut `s` to at most `max` bytes without splitting a UTF-8 character.
fn truncate_utf8(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Embed `text` under a nomic task prefix (`search_document: ` on add, `search_query: ` on recall).
/// Without the prefixes nomic similarities run ~0.15 low.
fn embed(prefix: &str, text: &str) -> Result<Vec<f32>, String> {
    let addr = std::env::var("INSTINCT_OLLAMA").map_err(|_| "optional embeddings disabled")?;
    let model = env_or("INSTINCT_EMBED_MODEL", DEFAULT_MODEL);
    let input = format!("{prefix}{}", truncate_utf8(text, MAX_EMBED_BYTES));
    let body = format!(
        "{{\"model\":\"{}\",\"input\":[\"{}\"]}}",
        escape_json(&model),
        escape_json(&input)
    );
    let resp = http_post_json(&addr, "/api/embed", &body)?;
    parse_embedding(&resp)
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let (mut dot, mut na, mut nb) = (0.0f32, 0.0f32, 0.0f32);
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom == 0.0 || !denom.is_finite() {
        return 0.0;
    }
    dot / denom
}

// ---------------------------------------------------------------- lexical (BM25) + fusion

const RRF_K: f32 = 60.0;

/// Lowercase alphanumeric/underscore words of >= 2 chars — keeps code identifiers (snake_case) whole.
fn tokens(s: &str) -> Vec<String> {
    s.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| w.chars().count() >= 2)
        .map(|w| w.to_lowercase())
        .collect()
}

/// Okapi BM25 (k1=1.2, b=0.75) of `query` against each document.
fn bm25(query: &str, docs: &[String]) -> Vec<f32> {
    use std::borrow::Cow;
    use std::collections::HashMap;

    let mut terms = tokens(query);
    terms.sort();
    terms.dedup();
    if terms.is_empty() || docs.is_empty() {
        return vec![0.0; docs.len()];
    }
    let indices: HashMap<&str, usize> = terms
        .iter()
        .enumerate()
        .map(|(i, term)| (term.as_str(), i))
        .collect();
    let mut df = vec![0usize; terms.len()];
    let mut total_length = 0usize;
    let mut corpus = Vec::with_capacity(docs.len());
    // Only query terms can contribute to the score. Count them in one pass,
    // preserving document lengths and the original sorted-term summation order.
    for doc in docs {
        let mut length = 0usize;
        let mut counts = HashMap::new();
        for word in doc.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
            if word.chars().nth(1).is_none() {
                continue;
            }
            length += 1;
            let lower: Cow<'_, str> =
                if word.is_ascii() && !word.bytes().any(|b| b.is_ascii_uppercase()) {
                    Cow::Borrowed(word)
                } else {
                    Cow::Owned(word.to_lowercase())
                };
            if let Some(&index) = indices.get(lower.as_ref()) {
                *counts.entry(index).or_insert(0usize) += 1;
            }
        }
        let mut matched: Vec<_> = counts.into_iter().collect();
        matched.sort_unstable_by_key(|&(i, _)| i);
        for &(i, _) in &matched {
            df[i] += 1;
        }
        total_length += length;
        corpus.push((length, matched));
    }
    let n = docs.len() as f32;
    let avgdl = (total_length as f32 / n).max(1.0);
    let idf: Vec<_> = df
        .into_iter()
        .map(|d| {
            let d = d as f32;
            ((n - d + 0.5) / (d + 0.5) + 1.0).ln()
        })
        .collect();
    corpus
        .into_iter()
        .map(|(length, matched)| {
            matched
                .into_iter()
                .map(|(i, count)| {
                    let tf = count as f32;
                    idf[i] * tf * 2.2 / (tf + 1.2 * (0.25 + 0.75 * length as f32 / avgdl))
                })
                .sum()
        })
        .collect()
}

/// Rank positions (0 = best) of the items with a positive score, best first.
fn ranking(scores: &[f32]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..scores.len()).filter(|&i| scores[i] > 0.0).collect();
    idx.sort_by(|&a, &b| {
        scores[b]
            .partial_cmp(&scores[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    idx
}

// ---------------------------------------------------------------- store

#[derive(Clone)]
struct Discovery {
    id: String,
    ts: u64,
    title: String,
    when: String,
    found: String,
    how: String,
    evidence: String,
    reconsider: String,
    tags: String,
    embedding: Vec<f32>,
    hits: u32,
    misses: u32,
    outcomes: HashMap<String, bool>,
    meta: eligibility::Meta,
}

fn apply_meta(m: &mut eligibility::Meta, v: &Json) {
    for (key, dest) in [
        ("source", &mut m.source),
        ("state", &mut m.state),
        ("evidence_status", &mut m.evidence_status),
        ("scope_project", &mut m.project),
        ("scope_persona", &mut m.persona),
        ("scope_harness", &mut m.harness),
        ("scope_version", &mut m.version),
    ] {
        if let Some(s) = v.get(key).and_then(Json::as_str) {
            *dest = s.to_string();
        }
    }
    if let Some(n) = v.get("valid_until").and_then(Json::as_f64) {
        m.valid_until = n as u64;
    }
}
fn meta_from(v: &Json) -> eligibility::Meta {
    let mut m = eligibility::Meta::default();
    apply_meta(&mut m, v);
    m
}

fn quiet_ids(_path: &PathBuf) -> HashSet<String> {
    let labels = std::env::var("INSTINCT_LABELS").ok();
    let mut counts = std::collections::HashMap::<String, (u32, u32)>::new();
    if let Some(text) = labels.and_then(|p| fs::read_to_string(p).ok()) {
        for line in text.lines() {
            if let Ok(v) = parse_json(line) {
                let id = v.get("id").and_then(Json::as_str).unwrap_or("");
                if id.is_empty() {
                    continue;
                }
                let e = counts.entry(id.to_string()).or_default();
                match v.get("verdict").and_then(Json::as_str).unwrap_or("") {
                    "helped" | "guarded" => e.0 += 1,
                    "irrelevant" | "misled" => e.1 += 1,
                    _ => {}
                }
            }
        }
    }
    counts
        .into_iter()
        .filter(|(_, (u, i))| eligibility::quiet(*u, *i))
        .map(|(id, _)| id)
        .collect()
}

impl Discovery {
    /// v2 2026-11-04 (lever-1, handover from cc-c9d58ab5): boost `when` 3× in the BM25 corpus.
    /// Recall @1 was 34.5 % because BM25 scored prose in `found`/`how` higher than the one field
    /// that actually describes the trigger context — `when`. Keep `found`+`how` so semantic phrase
    /// matches still land; triple `when` so prompt-text queries hit the right instinct more often.
    /// Embeddings computed on `d.text()` are cached per-entry; a `reembed` is needed to realign
    /// the semantic side (BM25 benefits immediately on next query).
    fn text(&self) -> String {
        format!(
            "{w} {w} {w} {t} {tg} {f} {h}",
            w = self.when,
            t = self.title,
            tg = self.tags,
            f = self.found,
            h = self.how
        )
    }
    /// 0.3.0-whenkey (2026-11-04): the BM25 side keys on the one-sentence `when` (ReasoningBank: the
    /// description IS the recall key). Field weights are env-tunable for benching — INSTINCT_W_WHEN
    /// (default 3) and INSTINCT_W_TITLE (default 1) — and `text()` above stays as the embedding input,
    /// so every stored vector remains aligned. With the defaults this equals `text()`.
    fn lex_text(&self) -> String {
        let ww = env_or("INSTINCT_W_WHEN", "3")
            .parse::<usize>()
            .unwrap_or(3)
            .clamp(0, 12);
        let wt = env_or("INSTINCT_W_TITLE", "1")
            .parse::<usize>()
            .unwrap_or(1)
            .clamp(0, 12);
        let body = env_or("INSTINCT_W_BODY", "1")
            .parse::<usize>()
            .unwrap_or(1)
            .clamp(0, 1);
        let mut s = String::with_capacity(
            self.when.len() * ww + self.title.len() * wt + self.found.len() + self.how.len() + 16,
        );
        for _ in 0..ww {
            s.push_str(&self.when);
            s.push(' ');
        }
        for _ in 0..wt {
            s.push_str(&self.title);
            s.push(' ');
        }
        s.push_str(&self.tags);
        s.push(' ');
        if body == 1 {
            s.push_str(&self.found);
            s.push(' ');
            s.push_str(&self.how);
        }
        s
    }
    /// Laplace-smoothed payoff rate: 0.5 for a fresh entry, -> 1.0 as recalls keep paying off.
    fn strength(&self) -> f32 {
        eligibility::strength(self.hits, self.misses)
    }
    fn to_line(&self) -> String {
        let emb: Vec<String> = self.embedding.iter().map(|f| f.to_string()).collect();
        let base = format!(
            "{{\"kind\":\"discovery\",\"id\":\"{}\",\"ts\":{},\"title\":\"{}\",\"when\":\"{}\",\"found\":\"{}\",\"how\":\"{}\",\"evidence\":\"{}\",\"tags\":\"{}\",\"reconsider\":\"{}\",\"embedding\":[{}]}}",
            escape_json(&self.id), self.ts, escape_json(&self.title), escape_json(&self.when), escape_json(&self.found),
            escape_json(&self.how), escape_json(&self.evidence), escape_json(&self.tags), escape_json(&self.reconsider), emb.join(",")
        );
        format!("{},\"source\":\"{}\",\"state\":\"{}\",\"evidence_status\":\"{}\",\"scope_project\":\"{}\",\"scope_persona\":\"{}\",\"scope_harness\":\"{}\",\"scope_version\":\"{}\",\"valid_until\":{}}}",
            base.trim_end_matches('}'), escape_json(&self.meta.source), escape_json(&self.meta.state), escape_json(&self.meta.evidence_status),
            escape_json(&self.meta.project),escape_json(&self.meta.persona),escape_json(&self.meta.harness),escape_json(&self.meta.version),self.meta.valid_until)
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn store_path(a: &Args) -> Result<PathBuf, String> {
    if let Some(p) = a.opt("store") {
        return Ok(PathBuf::from(p));
    }
    match std::env::var("INSTINCT_STORE") {
        Ok(p) if !p.is_empty() => Ok(PathBuf::from(p)),
        _ => Err("INSTINCT_STORE is not set (each agent has its own store; there is deliberately no default)".into()),
    }
}

/// Fold the append-only event log: discovery lines create entries, hit/miss lines count against them.
fn load(path: &PathBuf) -> Result<Vec<Discovery>, String> {
    let text = journal::read(path)?;
    let mut out: Vec<Discovery> = Vec::new();
    let mut superseded: Vec<(String, String)> = Vec::new();
    let mut retired: Vec<String> = Vec::new();
    // id -> position in `out`: every event row resolves its discovery in O(1) instead of a linear scan
    // (0.2.2: the scans cost ~6M string compares per load at 5.4k rows; dedupe added another ~2M).
    let mut idx: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (n, line) in text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
    {
        let v = parse_json(line).map_err(|e| format!("{}:{}: {e}", path.display(), n + 1))?;
        let s = |k: &str| v.get(k).and_then(Json::as_str).unwrap_or("").to_string();
        match v.get("kind").and_then(Json::as_str) {
            // 0.2.1-dedupe (2026-11-03): a block of 12 discovery rows was written twice. With a plain push the
            // `embed` merge below (first match) only reached the first copy, so `reembed` re-embedded the
            // empty second copy on every heal tick (273 embed rows per id, heal stuck at 1973/1980).
            // First row wins; a later copy of the same id only donates its vector when the first has none.
            Some("discovery") => {
                let d = Discovery {
                    id: s("id"),
                    ts: v.get("ts").and_then(Json::as_f64).unwrap_or(0.0) as u64,
                    title: s("title"),
                    when: s("when"),
                    found: s("found"),
                    how: s("how"),
                    evidence: s("evidence"),
                    reconsider: s("reconsider"),
                    tags: s("tags"),
                    embedding: v.get("embedding").and_then(json_floats).unwrap_or_default(),
                    hits: 0,
                    misses: 0,
                    outcomes: HashMap::new(),
                    meta: meta_from(&v),
                };
                if let Some(&i) = idx.get(&d.id) {
                    if out[i].embedding.is_empty() && !d.embedding.is_empty() {
                        out[i].embedding = d.embedding;
                    }
                } else {
                    idx.insert(d.id.clone(), out.len());
                    out.push(d);
                }
            }
            Some(k @ ("hit" | "miss")) => {
                let id = s("id");
                if let Some(&i) = idx.get(&id) {
                    let d = &mut out[i];
                    let paid_off = k == "hit";
                    if let Some(key) = v
                        .get("outcome_id")
                        .and_then(Json::as_str)
                        .filter(|k| !k.is_empty())
                    {
                        match d.outcomes.entry(key.to_owned()) {
                            std::collections::hash_map::Entry::Vacant(slot) => {
                                slot.insert(paid_off);
                            }
                            std::collections::hash_map::Entry::Occupied(_) => continue,
                        }
                    }
                    if paid_off {
                        d.hits += 1
                    } else {
                        d.misses += 1
                    }
                }
            }
            Some("review") => {
                if let Some(&i) = idx.get(&s("id")) {
                    apply_meta(&mut out[i].meta, &v);
                }
            }
            // A near-duplicate merged into a canonical lesson (consolidate): its hits/misses move to `by`, it leaves recall.
            Some("supersede") => superseded.push((s("id"), s("by"))),
            Some("unsupersede") => {
                let id = s("id");
                superseded.retain(|(m, _)| *m != id);
            }
            // Curated out from its wiki page (lapis-instinct-pages `curation: retire`): history stays, it leaves recall.
            Some("retire") => retired.push(s("id")),
            Some("unretire") => {
                let id = s("id");
                retired.retain(|m| *m != id);
            }
            // Cancels one earlier hit/miss (wrong-argument marks); history stays append-only.
            Some("retract") => {
                let id = s("id");
                if let Some(&i) = idx.get(&id) {
                    let d = &mut out[i];
                    match s("of").as_str() {
                        "hit" => d.hits = d.hits.saturating_sub(1),
                        "miss" => d.misses = d.misses.saturating_sub(1),
                        _ => {}
                    }
                }
            }
            // A later embedding for an entry banked while Ollama was down (written by `reembed`).
            Some("embed") => {
                let id = s("id");
                if let (Some(&i), Some(e)) =
                    (idx.get(&id), v.get("embedding").and_then(json_floats))
                {
                    if !e.is_empty() {
                        out[i].embedding = e
                    }
                }
            }
            _ => {}
        }
    }
    for (member, by) in &superseded {
        let Some(mi) = out.iter().position(|d| &d.id == member) else {
            continue;
        };
        let (h, m) = (out[mi].hits, out[mi].misses);
        if let Some(c) = out.iter_mut().find(|d| &d.id == by) {
            c.hits += h;
            c.misses += m;
        }
    }
    let gone: HashSet<&String> = superseded
        .iter()
        .map(|(m, _)| m)
        .chain(retired.iter())
        .collect();
    out.retain(|d| !gone.contains(&d.id));
    Ok(out)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Short stable id: FNV-1a over title+found, base36, prefixed "in-".
fn make_id(title: &str, found: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in title.bytes().chain([0u8]).chain(found.bytes()) {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    let mut s = String::new();
    let mut x = h;
    for _ in 0..7 {
        s.push(b"0123456789abcdefghijklmnopqrstuvwxyz"[(x % 36) as usize] as char);
        x /= 36;
    }
    format!("in-{s}")
}

// ---------------------------------------------------------------- args

struct Args {
    pos: Vec<String>,
    kv: Vec<(String, String)>,
}

impl Args {
    fn parse(argv: &[String]) -> Args {
        let (mut pos, mut kv) = (Vec::new(), Vec::new());
        let mut i = 0;
        while i < argv.len() {
            if let Some(k) = argv[i].strip_prefix("--") {
                // 0.3.0-whenkey: a `--switch` directly followed by another `--key` (or nothing) is a bare
                // switch and consumes no value — before this, `--fold --threshold 0.95` ate `--threshold`
                // and `--fold --apply` lost `--apply`. A value of "-" (stdin) is still a value.
                let next = argv.get(i + 1);
                if next.map_or(true, |n| n.starts_with("--")) {
                    kv.push((k.to_string(), String::new()));
                    i += 1;
                } else {
                    kv.push((k.to_string(), next.cloned().unwrap_or_default()));
                    i += 2;
                }
            } else {
                pos.push(argv[i].clone());
                i += 1;
            }
        }
        Args { pos, kv }
    }
    fn opt(&self, k: &str) -> Option<&str> {
        self.kv
            .iter()
            .rev()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.as_str())
    }
    fn text(&self, k: &str, stdin_used: &mut bool) -> Result<String, String> {
        match self.opt(k) {
            Some("-") if !*stdin_used => {
                *stdin_used = true;
                let mut s = String::new();
                std::io::stdin()
                    .read_to_string(&mut s)
                    .map_err(|e| format!("stdin: {e}"))?;
                Ok(s.trim().to_string())
            }
            Some(v) => Ok(v.to_string()),
            None => Ok(String::new()),
        }
    }
    fn num(&self, k: &str, default: usize) -> usize {
        self.opt(k).and_then(|v| v.parse().ok()).unwrap_or(default)
    }
    /// A bare switch (`--apply`): present at all, whatever value the parser paired with it.
    fn flag(&self, k: &str) -> bool {
        self.kv.iter().any(|(key, _)| key == k)
    }
}

// ---------------------------------------------------------------- operations (shared by CLI and MCP)

struct BankIn {
    title: String,
    when: String,
    found: String,
    how: String,
    evidence: String,
    reconsider: String,
    tags: String,
    project: String,
    persona: String,
    applicability_harness: String,
    applicability_version: String,
}

fn scope_identity(meta: &eligibility::Meta) -> [&str; 4] {
    [&meta.project, &meta.persona, &meta.harness, &meta.version]
}

fn scoped_bank_id(title: &str, found: &str, meta: &eligibility::Meta) -> String {
    let scope = scope_identity(meta);
    if scope.iter().all(|s| s.is_empty()) {
        return make_id(title, found);
    }
    // JSON supplies unambiguous field boundaries. The original short ID remains
    // for unscoped legacy-compatible writes; scoped facts get a separate identity.
    let body = json_out(&Json::Arr(
        [title, found, scope[0], scope[1], scope[2], scope[3]]
            .iter()
            .map(|s| Json::Str(s.to_string()))
            .collect(),
    ));
    let mut hash = 0xcbf29ce484222325u64;
    for b in body.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("in-s{hash:016x}")
}

fn bank_discovery(b: BankIn, context: &eligibility::Scope) -> Result<Discovery, String> {
    for (k, v, ex) in [
        ("title", &b.title, "Count delivered events"),
        (
            "when",
            &b.when,
            "Delivery and selection have separate timestamps",
        ),
        (
            "found",
            &b.found,
            "Only two selected items reached the recipient in time",
        ),
        (
            "how",
            &b.how,
            "Join event IDs and compare delivery with decision time",
        ),
        (
            "evidence",
            &b.evidence,
            "Five selected, two received before the decision",
        ),
    ] {
        if v.trim().is_empty() {
            return Err(format!(
                "--{k} is required (example: {ex}). A jackpot without its {k} is not reusable."
            ));
        }
    }
    let meta = eligibility::Meta {
        source: "banked".into(),
        state: "candidate".into(),
        evidence_status: "unverified".into(),
        project: if b.project.is_empty() {
            context.project.clone()
        } else {
            b.project
        },
        persona: if b.persona.is_empty() {
            context.persona.clone()
        } else {
            b.persona
        },
        // Receipt origin and applicability are distinct. Never infer these from
        // INSTINCT_HARNESS / INSTINCT_HARNESS_VERSION or the current caller.
        harness: b.applicability_harness,
        version: b.applicability_version,
        valid_until: 0,
    };
    let id = scoped_bank_id(&b.title, &b.found, &meta);
    Ok(Discovery {
        id,
        ts: now_secs(),
        title: b.title,
        when: b.when,
        found: b.found,
        how: b.how,
        evidence: b.evidence,
        reconsider: b.reconsider,
        tags: b.tags,
        embedding: Vec::new(),
        hits: 0,
        misses: 0,
        outcomes: HashMap::new(),
        meta,
    })
}

fn bank_scope_note(meta: &eligibility::Meta) -> String {
    let missing: Vec<_> = [
        ("project", &meta.project),
        ("persona", &meta.persona),
        ("applicability harness", &meta.harness),
        ("applicability version", &meta.version),
    ]
    .iter()
    .filter(|(_, v)| v.is_empty())
    .map(|(k, _)| *k)
    .collect();
    format!(
        "candidate, unverified{}",
        if missing.is_empty() {
            "; explicit applicability recorded".into()
        } else {
            format!(
                "; unspecified applicability (not isolated on these dimensions): {}",
                missing.join(", ")
            )
        }
    )
}

fn op_bank(path: &PathBuf, b: BankIn) -> Result<String, String> {
    let d0 = bank_discovery(b, &eligibility::env_scope())?;
    let id = d0.id.clone();
    let _lock = journal::identity_lock(path)?;
    if let Some(existing) = load(path)?.iter().find(|d| d.id == id) {
        if scope_identity(&existing.meta) != scope_identity(&d0.meta) {
            return Err(
                "bank identity collision across applicability scopes; nothing written".into(),
            );
        }
        return Ok(format!(
            "already banked: {id}; existing source/status preserved; requested {}",
            bank_scope_note(&d0.meta)
        ));
    }
    let scope_note = bank_scope_note(&d0.meta);
    let embedding = embed("search_document: ", &d0.text()).unwrap_or_default();
    let note = if embedding.is_empty() {
        " (no embedding: lexical recall for this entry)"
    } else {
        ""
    };
    append(path, &Discovery { embedding, ..d0 }.to_line())?;
    Ok(format!("banked {id}{note}; {scope_note}"))
}

fn render(d: &Discovery) -> String {
    format!(
        "[{}] {}  (strength {:.2}, {} hit / {} miss)\n  WHEN: {}\n  FOUND: {}\n  HOW: {}\n  EVIDENCE: {}{}",
        d.id, d.title, d.strength(), d.hits, d.misses, d.when, d.found, d.how, d.evidence,
        [("RECONSIDER", &d.reconsider), ("TAGS", &d.tags)].into_iter().filter(|(_,v)| !v.is_empty()).map(|(k,v)| format!("\n  {k}: {v}")).collect::<String>()
    )
}

// ---------------------------------------------------------------- `when` vectors (0.3.0-whenkey)
//
// A second vector per lesson, computed on the one-sentence `when` alone, kept in a SIDECAR
// `<store>.when.bin` (binary ID/vector records), so the main store's format and every 0.2.2
// fixture rule stay byte-identical. Built by `reembed --when` (append-only, only missing ids).
// Recall fuses it as a third RRF list when the sidecar exists; INSTINCT_WHEN_VEC=0 switches it off,
// deleting the sidecar is the full rollback.

/// Sidecar is BINARY (`<store>.when.bin`), append-only records: u16 id_len · id bytes · u16 dim · dim × f32 LE.
/// A text sidecar cost ~1.4 s per recall at 2k rows (measured 2026-11-04) and would breach the deny path's
/// 2 s cap; the binary load is a byte walk (< 10 ms). Rollback = delete the file.
fn when_sidecar(path: &PathBuf) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".when.bin");
    PathBuf::from(s)
}

/// id → `when` vector; later records for the same id win (append-only re-embeds).
fn load_when(path: &PathBuf) -> std::collections::HashMap<String, Vec<f32>> {
    let mut out = std::collections::HashMap::new();
    let Ok(buf) = fs::read(when_sidecar(path)) else {
        return out;
    };
    let mut p = 0usize;
    while p + 4 <= buf.len() {
        let il = u16::from_le_bytes([buf[p], buf[p + 1]]) as usize;
        p += 2;
        if p + il + 2 > buf.len() {
            break;
        }
        let id = String::from_utf8_lossy(&buf[p..p + il]).to_string();
        p += il;
        let dim = u16::from_le_bytes([buf[p], buf[p + 1]]) as usize;
        p += 2;
        if p + dim * 4 > buf.len() {
            break;
        }
        let mut v = Vec::with_capacity(dim);
        for i in 0..dim {
            let o = p + i * 4;
            v.push(f32::from_le_bytes([
                buf[o],
                buf[o + 1],
                buf[o + 2],
                buf[o + 3],
            ]));
        }
        p += dim * 4;
        if !v.is_empty() && v.iter().all(|x| x.is_finite()) {
            out.insert(id, v);
        }
    }
    out
}

fn append_when(path: &PathBuf, id: &str, v: &[f32]) -> Result<(), String> {
    let side = when_sidecar(path);
    let mut rec: Vec<u8> = Vec::with_capacity(4 + id.len() + v.len() * 4);
    rec.extend_from_slice(&(id.len() as u16).to_le_bytes());
    rec.extend_from_slice(id.as_bytes());
    rec.extend_from_slice(&(v.len() as u16).to_le_bytes());
    for x in v {
        rec.extend_from_slice(&x.to_le_bytes());
    }
    let mut f = journal::private_options()
        .create(true)
        .append(true)
        .open(&side)
        .map_err(|e| format!("cannot open {}: {e}", side.display()))?;
    f.write_all(&rec)
        .map_err(|e| format!("write {} failed: {e}", side.display()))
}

/// Embed `when` for every live entry lacking a when-vector; appends to the sidecar.
fn op_reembed_when(path: &PathBuf) -> Result<String, String> {
    let all = load(path)?;
    let have = load_when(path);
    let todo: Vec<&Discovery> = all
        .iter()
        .filter(|d| !have.contains_key(&d.id) && !d.when.trim().is_empty())
        .collect();
    let mut done = 0;
    for d in &todo {
        let e = embed("search_document: ", &d.when)
            .map_err(|e| format!("stopped after {done}/{} ({e})", todo.len()))?;
        append_when(path, &d.id, &e)?;
        done += 1;
    }
    Ok(format!(
        "when-embedded {done} of {} entries without a when-vector → {}",
        todo.len(),
        when_sidecar(path).display()
    ))
}

fn op_recall(path: &PathBuf, q: &str, k: usize) -> Result<String, String> {
    op_recall_scoped(path, q, k, &eligibility::env_scope())
}
fn op_recall_scoped(
    path: &PathBuf,
    q: &str,
    k: usize,
    scope: &eligibility::Scope,
) -> Result<String, String> {
    op_recall_data(path, q, k, scope).map(|(text, _)| text)
}
// Directly banked discoveries can be tried offline before their first payoff.
fn anchors(text: &str) -> HashSet<String> {
    tokens(text)
        .into_iter()
        .filter(|w| {
            w.len() >= 4
                && !matches!(
                    w.as_str(),
                    "with"
                        | "when"
                        | "that"
                        | "this"
                        | "from"
                        | "what"
                        | "which"
                        | "have"
                        | "does"
                        | "need"
                        | "using"
                        | "into"
                        | "were"
                        | "there"
                        | "their"
                        | "they"
                        | "before"
                        | "after"
                )
        })
        .collect()
}

fn recall_methods(
    path: &PathBuf,
    q: &str,
    k: usize,
    scope: &eligibility::Scope,
) -> Result<Vec<Discovery>, String> {
    let all = load_scoped(path, scope, now_secs())?;
    let lex = bm25(q, &all.iter().map(Discovery::lex_text).collect::<Vec<_>>());
    rank_methods(path, q, k, scope, &all, lex)
}

fn load_scoped(
    path: &PathBuf,
    scope: &eligibility::Scope,
    now: u64,
) -> Result<Vec<Discovery>, String> {
    Ok(load(path)?
        .into_iter()
        .filter(|d| eligibility::scope_check(&d.meta, scope, now).is_ok())
        .collect())
}

fn rank_methods(
    path: &PathBuf,
    q: &str,
    k: usize,
    scope: &eligibility::Scope,
    all: &[Discovery],
    lex: Vec<f32>,
) -> Result<Vec<Discovery>, String> {
    if all.is_empty() || q.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut fused = vec![0f32; all.len()];
    let mut similarities: Vec<Option<f32>> = vec![None; all.len()];
    let quiet = quiet_ids(path);
    let w_lex: f32 = env_or("INSTINCT_W_LEX", "1").parse().unwrap_or(1.0);
    for (r, &i) in ranking(&lex).iter().enumerate() {
        fused[i] += w_lex / (RRF_K + r as f32 + 1.0);
    }
    let mode = env_or("INSTINCT_VEC_MODE", "both");
    let whenv = if env_or("INSTINCT_WHEN_VEC", "1") == "0" || mode == "full" {
        std::collections::HashMap::new()
    } else {
        load_when(path)
    };
    if let Ok(qe) = embed("search_query: ", q) {
        for (i, d) in all.iter().enumerate() {
            if !d.embedding.is_empty() {
                similarities[i] = Some(cosine(&qe, &d.embedding));
            }
        }
        if mode != "when" || whenv.is_empty() {
            let sem: Vec<f32> = all
                .iter()
                .map(|d| {
                    let c = cosine(&qe, &d.embedding);
                    if c >= 0.5 {
                        c
                    } else {
                        0.0
                    }
                })
                .collect();
            for (r, &i) in ranking(&sem).iter().enumerate() {
                fused[i] += 1.0 / (RRF_K + r as f32 + 1.0);
            }
        }
        // Two guards keep the when-vector from diluting raw-command recall (measured 2026-11-04: as a free third
        // list it cost -4 @5 on 46 shell-command situations while gaining +2 @1):
        //  · INSTINCT_WHENVEC_GATE=1  — the when-vector only re-ranks items an existing signal already found
        //    (BM25 > 0 or full-vector cosine >= 0.5); it cannot introduce candidates of its own.
        //  · INSTINCT_WHENVEC_NLONLY=1 — skip it when the query reads as a shell command (>= 30 % of tokens
        //    carry / - $ | & ; = or start with a digit); situations are prose, deny-path queries are commands.
        let shellish = {
            let toks: Vec<&str> = q.split_whitespace().collect();
            let n = toks.len().max(1) as f32;
            let s = toks
                .iter()
                .filter(|t| {
                    t.contains('/')
                        || t.starts_with('-')
                        || t.contains('$')
                        || t.contains('|')
                        || t.contains('&')
                        || t.contains(';')
                        || t.contains('=')
                        || t.starts_with(|c: char| c.is_ascii_digit())
                })
                .count() as f32;
            s / n >= 0.30
        };
        let nl_only = env_or("INSTINCT_WHENVEC_NLONLY", "0") == "1";
        if !whenv.is_empty() && !(nl_only && shellish) {
            let w: f32 = env_or("INSTINCT_W_WHENVEC", "1").parse().unwrap_or(1.0);
            let gate = env_or("INSTINCT_WHENVEC_GATE", "0") == "1";
            let semw: Vec<f32> = all
                .iter()
                .enumerate()
                .map(|(i, d)| {
                    if gate && lex[i] <= 0.0 && cosine(&qe, &d.embedding) < 0.5 {
                        return 0.0;
                    }
                    whenv
                        .get(&d.id)
                        .map(|v| {
                            let c = cosine(&qe, v);
                            if c >= 0.5 {
                                c
                            } else {
                                0.0
                            }
                        })
                        .unwrap_or(0.0)
                })
                .collect();
            for (r, &i) in ranking(&semw).iter().enumerate() {
                fused[i] += w / (RRF_K + r as f32 + 1.0);
            }
        }
    }
    for (i, d) in all.iter().enumerate() {
        if fused[i] > 0.0 {
            fused[i] += 0.002 * d.strength();
        }
    }
    // Result fold (INSTINCT_FOLD = cosine threshold, default off): a near-duplicate of a lesson already in
    // the answer does not take a slot, so top-k carries k DIFFERENT lessons.
    let fold: f32 = env_or("INSTINCT_FOLD", "0").parse().unwrap_or(0.0);
    let mut picked: Vec<usize> = Vec::with_capacity(k.min(all.len()));
    let cues = anchors(q);
    for i in ranking(&fused) {
        if picked.len() >= k {
            break;
        }
        let verdict = eligibility::eligible(
            &all[i].meta,
            scope,
            now_secs(),
            similarities[i],
            all[i].hits,
            quiet.contains(&all[i].id),
        );
        let trigger_matches =
            |trigger: &str| anchors(trigger).intersection(&cues).take(2).count() == 2;
        let strong_trigger =
            trigger_matches(&all[i].when) || all[i].tags.split(';').any(trigger_matches);
        let offline_candidate = verdict == Err("candidate_match_unproven")
            && all[i].meta.source == "banked"
            && strong_trigger;
        if (verdict.is_err() && !offline_candidate)
            || eligibility::quiet(all[i].hits, all[i].misses)
            || (!strong_trigger && similarities[i].unwrap_or(0.0) < 0.74)
        {
            continue;
        }
        if fold > 0.0
            && !all[i].embedding.is_empty()
            && picked.iter().any(|&j| {
                !all[j].embedding.is_empty() && cosine(&all[i].embedding, &all[j].embedding) >= fold
            })
        {
            continue;
        }
        picked.push(i);
    }
    if picked.is_empty() && k > 0 && cues.len() >= 2 {
        let query = tokens(q);
        let mut matches = Vec::new();
        for (i, d) in all.iter().enumerate() {
            if d.hits == 0
                || quiet.contains(&d.id)
                || eligibility::quiet(d.hits, d.misses)
                || eligibility::scope_check(&d.meta, scope, now_secs()).is_err()
            {
                continue;
            }
            let score = std::iter::once(d.when.as_str())
                .chain(d.tags.split(';'))
                .map(|cue| helix_db::cue_similarity(&query, cue))
                .fold(0.0_f64, f64::max);
            if score > 0.0 {
                matches.push((i, score));
            }
        }
        matches.sort_by(|a, b| b.1.total_cmp(&a.1));
        if let Some(&(i, score)) = matches.first() {
            if score >= 0.62 && matches.get(1).is_none_or(|next| score - next.1 >= 0.08) {
                picked.push(i);
            }
        }
    }
    Ok(picked.into_iter().map(|i| all[i].clone()).collect())
}

fn op_recall_data(
    path: &PathBuf,
    q: &str,
    k: usize,
    scope: &eligibility::Scope,
) -> Result<(String, Vec<String>), String> {
    Ok(render_recall(&recall_methods(path, q, k, scope)?))
}

fn render_recall(methods: &[Discovery]) -> (String, Vec<String>) {
    let ids = methods.iter().map(|d| d.id.clone()).collect();
    let text = methods
        .iter()
        .map(|d| {
            format!(
                "{}\n  VALIDITY: {}{}",
                render(d),
                if eligibility::unverified(&d.meta) {
                    "source claim not independently verified"
                } else {
                    "verified within recorded scope"
                },
                if d.meta.state == "candidate" || d.meta.source == "mined" {
                    "; candidate, check original evidence"
                } else {
                    ""
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    (text, ids)
}

/// A synchronous adapter for any host's pre-decision step. No model call or feedback side effect.
fn decision_context(
    path: &PathBuf,
    situation: &str,
    scope: &eligibility::Scope,
) -> Result<String, String> {
    if situation.trim().is_empty() {
        return Err("decide requires a situation via --q".into());
    }
    let methods = recall_methods(path, situation, 2, scope)?;
    let experiences = methods
        .iter()
        .map(|d| {
            Json::Obj(vec![
                ("id".into(), Json::Str(d.id.clone())),
                ("when".into(), Json::Str(d.when.clone())),
                ("method".into(), Json::Str(d.how.clone())),
                ("observed".into(), Json::Str(d.found.clone())),
                ("evidence".into(), Json::Str(d.evidence.clone())),
                (
                    "reconsider_if".into(),
                    if d.reconsider.is_empty() {
                        Json::Null
                    } else {
                        Json::Str(d.reconsider.clone())
                    },
                ),
                ("hits".into(), Json::Num(d.hits.into())),
                ("misses".into(), Json::Num(d.misses.into())),
                (
                    "evidence_status".into(),
                    Json::Str(
                        if eligibility::unverified(&d.meta) {
                            "unverified"
                        } else {
                            "verified_in_scope"
                        }
                        .into(),
                    ),
                ),
            ])
        })
        .collect();
    Ok(json_out(&Json::Obj(vec![
        ("version".into(), Json::Num(1.0)),
        ("experiences".into(), Json::Arr(experiences)),
    ])))
}

fn op_mark(
    path: &PathBuf,
    kind: &str,
    id: &str,
    note: &str,
    outcome_id: &str,
) -> Result<String, String> {
    let _lock = journal::identity_lock(path)?;
    let entry = load(path)?
        .into_iter()
        .find(|d| d.id == id)
        .ok_or_else(|| format!("no discovery with id {id}"))?;
    let key = outcome_id.trim();
    if let Some(&paid_off) = entry.outcomes.get(key) {
        if paid_off != (kind == "hit") {
            return Err("outcome already recorded with a different verdict".into());
        }
        return Ok(format!("outcome already recorded for {id}"));
    }
    let mut fields = vec![
        ("kind".into(), Json::Str(kind.into())),
        ("id".into(), Json::Str(id.into())),
        ("ts".into(), Json::Num(now_secs() as f64)),
        ("note".into(), Json::Str(note.into())),
    ];
    if !key.is_empty() {
        fields.push(("outcome_id".into(), Json::Str(key.into())));
    }
    append(path, &json_out(&Json::Obj(fields)))?;
    Ok(format!("{kind} recorded for {id}"))
}

fn op_review(
    path: &PathBuf,
    id: &str,
    state: &str,
    reason: &str,
    reference: &str,
) -> Result<String, String> {
    if !["quarantined", "candidate"].contains(&state)
        || reason.trim().is_empty()
        || reference.trim().is_empty()
    {
        return Err("review requires state quarantined|candidate, reason and source-ref; does not grant verified status".into());
    }
    if !load(path)?.iter().any(|d| d.id == id) {
        return Err("unknown active lesson".into());
    }
    append(path,&format!("{{\"kind\":\"review\",\"id\":\"{}\",\"ts\":{},\"state\":\"{state}\",\"evidence_status\":\"unverified\",\"reason\":\"{}\",\"source_ref\":\"{}\"}}",escape_json(id),now_secs(),escape_json(reason),escape_json(reference)))?;
    Ok(format!("reviewed {id}: {state}; original preserved"))
}

/// Instincts = discoveries that kept paying off: >= min_hits hits and strength >= 0.6. One reflex line each.
fn op_instincts(path: &PathBuf, min_hits: u32) -> Result<String, String> {
    let scope = eligibility::env_scope();
    let quiet = quiet_ids(path);
    let mut all: Vec<Discovery> = load(path)?
        .into_iter()
        .filter(|d| {
            eligibility::promoted(d.hits, d.misses, min_hits)
                && eligibility::eligible(
                    &d.meta,
                    &scope,
                    now_secs(),
                    None,
                    d.hits,
                    quiet.contains(&d.id),
                )
                .is_ok()
        })
        .collect();
    all.sort_by(|a, b| {
        b.strength()
            .partial_cmp(&a.strength())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.hits.cmp(&a.hits))
    });
    Ok(all.iter().map(|d| format!("- WHEN {} → {} (how: {}) [{} · {} hits; historical source claim, verify applicability]", d.when, d.found, d.how, d.id, d.hits)).collect::<Vec<_>>().join("\n"))
}

/// Embed every entry that has no vector yet (banked while Ollama was down); appends one `embed` event per entry.
fn op_reembed(path: &PathBuf) -> Result<String, String> {
    let all = load(path)?;
    let todo: Vec<&Discovery> = all.iter().filter(|d| d.embedding.is_empty()).collect();
    let mut done = 0;
    for d in &todo {
        let e = embed("search_document: ", &d.text())
            .map_err(|e| format!("stopped after {done}/{} ({e})", todo.len()))?;
        let emb: Vec<String> = e.iter().map(|f| f.to_string()).collect();
        append(
            path,
            &format!(
                "{{\"kind\":\"embed\",\"id\":\"{}\",\"ts\":{},\"embedding\":[{}]}}",
                escape_json(&d.id),
                now_secs(),
                emb.join(",")
            ),
        )?;
        done += 1;
    }
    Ok(format!(
        "re-embedded {done} of {} entries without a vector",
        todo.len()
    ))
}

fn op_stats(path: &PathBuf) -> Result<String, String> {
    let all = load(path)?;
    let inst = all
        .iter()
        .filter(|d| eligibility::promoted(d.hits, d.misses, 2))
        .count();
    let emb = all.iter().filter(|d| !d.embedding.is_empty()).count();
    let hits: u32 = all.iter().map(|d| d.hits).sum();
    let misses: u32 = all.iter().map(|d| d.misses).sum();
    Ok(format!(
        "store {} · {} discoveries ({} embedded) · {} instincts · {} hits / {} misses",
        path.display(),
        all.len(),
        emb,
        inst,
        hits,
        misses
    ))
}

/// Re-serialise a parsed Json value.
fn json_out(v: &Json) -> String {
    match v {
        Json::Null => "null".into(),
        Json::Bool(b) => b.to_string(),
        Json::Num(n) => n.to_string(),
        Json::Str(s) => format!("\"{}\"", escape_json(s)),
        Json::Arr(a) => format!("[{}]", a.iter().map(json_out).collect::<Vec<_>>().join(",")),
        Json::Obj(f) => format!(
            "{{{}}}",
            f.iter()
                .map(|(k, v)| format!("\"{}\":{}", escape_json(k), json_out(v)))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

// ---------------------------------------------------------------- MCP (JSON-RPC 2.0 over newline-delimited stdio)

fn tool_schema() -> &'static str {
    r#"[{"name":"instinct_bank","description":"Bank a discovery and its method as an unverified candidate. Supply applicability explicitly: omitted dimensions remain unspecified, not verified global applicability. Evidence text does not grant verified status.","inputSchema":{"type":"object","properties":{"title":{"type":"string"},"when":{"type":"string","description":"the situation/trigger where this applies"},"found":{"type":"string","description":"what was discovered (the fact/fix/lever)"},"how":{"type":"string","description":"the method that found it (probe, measurement, question asked)"},"evidence":{"type":"string","description":"the measured numbers or verification"},"reconsider":{"type":"string","description":"observed or explicitly stated change that would make this method unsuitable"},"tags":{"type":"string"},"project":{"type":"string","description":"exact project applicability; falls back only to explicit INSTINCT_PROJECT"},"persona":{"type":"string","description":"exact persona applicability; falls back only to explicit INSTINCT_PERSONA"},"applicability_harness":{"type":"string","description":"harness this lesson applies to, not the caller or receipt origin"},"applicability_version":{"type":"string","description":"exact applicable harness version, if known"}},"required":["title","when","found","how","evidence"]}},
{"name":"instinct_recall","description":"Recall scoped discoveries. Pass project, persona and harness when known; unspecified context does not match explicitly scoped lessons. Returned claims still require evidence checks.","inputSchema":{"type":"object","properties":{"q":{"type":"string"},"k":{"type":"integer"},"project":{"type":"string"},"persona":{"type":"string"},"harness":{"type":"string"},"harness_version":{"type":"string"},"session":{"type":"string"}},"required":["q"]}},
{"name":"instinct_feedback","description":"Record whether a recalled discovery paid off (hit) or not (miss). Hits promote a discovery to an instinct.","inputSchema":{"type":"object","properties":{"id":{"type":"string"},"paid_off":{"type":"boolean"},"note":{"type":"string"},"outcome_id":{"type":"string","description":"Reuse the same key when retrying feedback for one observed outcome."}},"required":["id","paid_off"]}},
{"name":"instinct_list","description":"List instincts: discoveries that repeatedly paid off, as WHEN -> DO reflex lines.","inputSchema":{"type":"object","properties":{}}}]"#
}

fn recall_receipt(picked: &[String], args: Option<&Json>) -> Option<Json> {
    let ids: Vec<Json> = picked.iter().map(|id| Json::Str(id.clone())).collect();
    if ids.is_empty() {
        return None;
    }
    let arg = |k: &str| {
        args.and_then(|v| v.get(k))
            .and_then(Json::as_str)
            .unwrap_or("")
            .to_string()
    };
    let mut session = arg("session");
    if session.is_empty() {
        session = env_or("INSTINCT_SESSION", "unknown-session");
    }
    let mut harness = arg("harness");
    if harness.is_empty() {
        harness = env_or("INSTINCT_HARNESS", "unknown");
    }
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    Some(Json::Obj(vec![
        ("schema_version".into(), Json::Num(1.0)),
        (
            "event_id".into(),
            Json::Str(format!("recall-{}-{ns}", std::process::id())),
        ),
        ("ts".into(), Json::Num(now_secs() as f64)),
        ("session".into(), Json::Str(session)),
        ("harness".into(), Json::Str(harness)),
        ("ids".into(), Json::Arr(ids)),
        ("channel".into(), Json::Str("mcp-recall".into())),
        (
            "delivery_boundary".into(),
            Json::Str("MCP stdout flushed; model use unproven".into()),
        ),
    ]))
}
fn receipt_stage(mut meta: Json, stage: &str, text: &str) -> Result<(), String> {
    // Opt in per installation. Separate stores do not write into another harness's telemetry.
    let root = match std::env::var("INSTINCT_TELEMETRY_DIR") {
        Ok(s) if !s.is_empty() => PathBuf::from(s),
        _ => return Ok(()),
    };
    let mut child = std::process::Command::new("/usr/bin/sha256sum")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .ok_or("hash stdin")?
        .write_all(text.as_bytes())
        .map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let hash = String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_string();
    if !out.status.success() || hash.len() != 64 {
        return Err("receipt hashing failed".into());
    }
    if let Json::Obj(ref mut fields) = meta {
        fields.push(("stage".into(), Json::Str(stage.into())));
        fields.push(("excerpt_sha256".into(), Json::Str(hash)));
    }
    append(
        &root.join(if stage == "selected" {
            "instinct-injections.jsonl"
        } else {
            "instinct-deliveries.jsonl"
        }),
        &json_out(&meta),
    )
}

fn mcp_call(
    cache: &mut Option<recall_index::Index>,
    path: &PathBuf,
    name: &str,
    args: Option<&Json>,
) -> Result<(String, Vec<String>), String> {
    let s = |k: &str| {
        args.and_then(|a| a.get(k))
            .and_then(Json::as_str)
            .unwrap_or("")
            .to_string()
    };
    let out = match name {
        "instinct_bank" => op_bank(
            path,
            BankIn {
                title: s("title"),
                when: s("when"),
                found: s("found"),
                how: s("how"),
                evidence: s("evidence"),
                reconsider: s("reconsider"),
                tags: s("tags"),
                project: s("project"),
                persona: s("persona"),
                applicability_harness: s("applicability_harness"),
                applicability_version: s("applicability_version"),
            },
        ),
        "instinct_recall" => {
            let k = args
                .and_then(|a| a.get("k"))
                .and_then(Json::as_f64)
                .unwrap_or(5.0) as usize;
            let mut scope = eligibility::env_scope();
            for (key, dest) in [
                ("project", &mut scope.project),
                ("persona", &mut scope.persona),
                ("harness", &mut scope.harness),
                ("harness_version", &mut scope.version),
            ] {
                let v = s(key);
                if !v.is_empty() {
                    *dest = v;
                }
            }
            return recall_index::recall(cache, path, &s("q"), k.clamp(1, 20), &scope).map(
                |methods| {
                    let (r, ids) = render_recall(&methods);
                    (
                        if r.is_empty() {
                            "(nothing banked matches)".into()
                        } else {
                            r
                        },
                        ids,
                    )
                },
            );
        }
        "instinct_feedback" => {
            // A missing or misspelled flag used to record a silent "miss" (2026-10-20: `paid: true` became a miss, and old
            // misses carry notes that read like wins). Now: accept the obvious spellings, refuse anything else.
            let flag = ["paid_off", "paid", "helped", "hit"]
                .iter()
                .find_map(|k| args.and_then(|a| a.get(k)));
            let paid = match flag {
                Some(Json::Bool(b)) => *b,
                _ => return Err("instinct_feedback needs paid_off: true (helped) or false (misled); nothing recorded".into()),
            };
            op_mark(
                path,
                if paid { "hit" } else { "miss" },
                &s("id"),
                &s("note"),
                &s("outcome_id"),
            )
        }
        "instinct_list" => op_instincts(path, 2).map(|r| {
            if r.is_empty() {
                "(no instincts yet — discoveries become instincts after 2+ hits)".into()
            } else {
                r
            }
        }),
        _ => Err(format!("unknown tool {name}")),
    };
    out.map(|text| (text, Vec::new()))
}

fn mcp(path: &PathBuf) -> Result<(), String> {
    let mut cache = None;
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| format!("stdin: {e}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(msg) = parse_json(&line) else { continue };
        let Some(id) = msg.get("id") else { continue }; // notifications get no reply
        let id_txt = match id {
            Json::Num(n) => format!("{}", *n as i64),
            Json::Str(s) => format!("\"{}\"", escape_json(s)),
            _ => "null".into(),
        };
        let method = msg.get("method").and_then(Json::as_str).unwrap_or("");
        let mut delivery: Option<(Json, String)> = None;
        let result = match method {
            "initialize" => {
                let pv = msg
                    .get("params")
                    .and_then(|p| p.get("protocolVersion"))
                    .and_then(Json::as_str)
                    .unwrap_or("2024-11-05");
                format!("{{\"protocolVersion\":\"{}\",\"capabilities\":{{\"tools\":{{}}}},\"serverInfo\":{{\"name\":\"instinct\",\"version\":\"{}\"}}}}", escape_json(pv), env!("CARGO_PKG_VERSION"))
            }
            "tools/list" => format!("{{\"tools\":{}}}", tool_schema().replace('\n', "")),
            "tools/call" => {
                let p = msg.get("params");
                let name = p
                    .and_then(|p| p.get("name"))
                    .and_then(Json::as_str)
                    .unwrap_or("");
                let (text, picked, err) =
                    match mcp_call(&mut cache, path, name, p.and_then(|p| p.get("arguments"))) {
                        Ok((t, ids)) => (t, ids, false),
                        Err(e) => (e, Vec::new(), true),
                    };
                if name == "instinct_recall" && !err {
                    if let Some(meta) = recall_receipt(&picked, p.and_then(|p| p.get("arguments")))
                    {
                        if let Err(e) = receipt_stage(meta.clone(), "selected", &text) {
                            eprintln!("instinct telemetry: {e}");
                        }
                        delivery = Some((meta, text.clone()));
                    }
                }
                format!(
                    "{{\"content\":[{{\"type\":\"text\",\"text\":\"{}\"}}],\"isError\":{err}}}",
                    escape_json(&text)
                )
            }
            "ping" => "{}".into(),
            _ => {
                writeln!(out, "{{\"jsonrpc\":\"2.0\",\"id\":{id_txt},\"error\":{{\"code\":-32601,\"message\":\"method not found\"}}}}").ok();
                out.flush().ok();
                continue;
            }
        };
        writeln!(
            out,
            "{{\"jsonrpc\":\"2.0\",\"id\":{id_txt},\"result\":{result}}}"
        )
        .map_err(|e| format!("stdout: {e}"))?;
        out.flush().map_err(|e| format!("stdout flush: {e}"))?;
        if let Some((meta, text)) = delivery {
            if let Err(e) = receipt_stage(meta, "delivered", &text) {
                eprintln!("instinct telemetry: {e}");
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- main

const USAGE: &str = "usage: migrate --store OLD.jsonl --to NEW.hxdb | export --store BANK | demo [SITUATION] | bank --title T --when W --found F --how H --evidence E [--tags a,b] [--project P] [--persona P] [--applicability-harness H] [--applicability-version V] | decide --q SITUATION | recall --q Q [--k 5] | hit|miss ID [--note N] [--outcome-id K] | instincts [--min-hits 2] | stats | reembed [--when] | retract hit|miss ID | review ID --state quarantined --reason WHY --source-ref REF | unsupersede ID | prompt | mcp";

fn run(argv: &[String]) -> Result<(), String> {
    let cmd = argv.first().map(String::as_str).unwrap_or("");
    if matches!(cmd, "" | "--help" | "-h" | "help") {
        println!("{USAGE}");
        return Ok(());
    }
    if matches!(cmd, "--version" | "-V") {
        println!("instinct {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if cmd == "demo" {
        if argv.len() > 2 {
            return Err("usage: instinct demo [quoted situation]".into());
        }
        return demo::run(argv.get(1).map(String::as_str));
    }
    let a = Args::parse(argv.get(1..).unwrap_or(&[]));
    let path = store_path(&a)?;
    for (key, env) in [
        ("project", "INSTINCT_PROJECT"),
        ("persona", "INSTINCT_PERSONA"),
        ("harness", "INSTINCT_HARNESS"),
        ("harness-version", "INSTINCT_HARNESS_VERSION"),
    ] {
        if let Some(v) = a.opt(key) {
            std::env::set_var(env, v);
        }
    }
    let mut stdin_used = false;
    let out = match cmd {
        "migrate" => {
            if !path.is_file() {
                return Err("source bank does not exist".into());
            }
            let dest = PathBuf::from(a.opt("to").ok_or("supply --to NEW.hxdb")?);
            let snapshot = journal::read(&path)?;
            let count = helix_db::import(&dest, &snapshot)?;
            format!("migrated {count} events to {}", dest.display())
        }
        "export" => {
            if !path.is_file() {
                return Err("source bank does not exist".into());
            }
            let snapshot = journal::read(&path)?;
            for line in snapshot.lines().filter(|s| !s.trim().is_empty()) {
                parse_json(line)?;
            }
            print!("{snapshot}");
            return Ok(());
        }
        "bank" => op_bank(
            &path,
            BankIn {
                title: a.text("title", &mut stdin_used)?,
                when: a.text("when", &mut stdin_used)?,
                found: a.text("found", &mut stdin_used)?,
                how: a.text("how", &mut stdin_used)?,
                evidence: a.text("evidence", &mut stdin_used)?,
                reconsider: a.text("reconsider", &mut stdin_used)?,
                tags: a.text("tags", &mut stdin_used)?,
                project: a.text("project", &mut stdin_used)?,
                persona: a.text("persona", &mut stdin_used)?,
                applicability_harness: a.text("applicability-harness", &mut stdin_used)?,
                applicability_version: a.text("applicability-version", &mut stdin_used)?,
            },
        )?,
        "decide" => decision_context(
            &path,
            &a.text("q", &mut stdin_used)?,
            &eligibility::env_scope(),
        )?,
        "prompt" => {
            let mut input = String::new();
            std::io::stdin()
                .read_to_string(&mut input)
                .map_err(|e| e.to_string())?;
            let event = parse_json(&input)?;
            let query = event.get("prompt").and_then(Json::as_str).unwrap_or("");
            let mut scope = eligibility::env_scope();
            if let Some(cwd) = event.get("cwd").and_then(Json::as_str) {
                scope.project = cwd.to_owned();
            }
            op_recall_scoped(&path, query, 1, &scope)?
        }
        "recall" => op_recall(&path, &a.text("q", &mut stdin_used)?, a.num("k", 5))?,
        "review" => op_review(
            &path,
            a.pos.first().ok_or("review ID required")?,
            a.opt("state").unwrap_or(""),
            a.opt("reason").unwrap_or(""),
            a.opt("source-ref").unwrap_or(""),
        )?,
        "hit" | "miss" => op_mark(
            &path,
            cmd,
            a.pos.first().ok_or("usage: hit|miss ID [--note N]")?,
            a.opt("note").unwrap_or(""),
            a.opt("outcome-id").unwrap_or(""),
        )?,
        "instincts" => op_instincts(&path, a.num("min-hits", 2) as u32)?,
        "stats" => op_stats(&path)?,
        "reembed" => {
            if a.flag("when") {
                op_reembed_when(&path)?
            } else {
                op_reembed(&path)?
            }
        }
        "unsupersede" => {
            let id = a.pos.first().ok_or("usage: unsupersede ID [--note N]")?;
            append(
                &path,
                &format!(
                    "{{\"kind\":\"unsupersede\",\"id\":\"{}\",\"ts\":{},\"note\":\"{}\"}}",
                    escape_json(id),
                    now_secs(),
                    escape_json(a.opt("note").unwrap_or(""))
                ),
            )?;
            format!("unsuperseded {id} (back in recall)")
        }
        "retract" => {
            let of = a.pos.first().map(String::as_str).unwrap_or("");
            let id = a
                .pos
                .get(1)
                .ok_or("usage: retract hit|miss ID [--note N]")?;
            if of != "hit" && of != "miss" {
                return Err("usage: retract hit|miss ID [--note N]".into());
            }
            append(&path, &format!("{{\"kind\":\"retract\",\"of\":\"{of}\",\"id\":\"{}\",\"ts\":{},\"note\":\"{}\"}}", escape_json(id), now_secs(), escape_json(a.opt("note").unwrap_or(""))))?;
            format!("retracted one {of} on {id}")
        }
        "mcp" => return mcp(&path),
        _ => return Err(USAGE.into()),
    };
    if !out.is_empty() {
        println!("{out}");
    }
    Ok(())
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run(&argv) {
        eprintln!("instinct: {e}");
        process::exit(2);
    }
}

#[cfg(test)]
mod reliable_tests {
    use super::*;
    fn bank_input() -> BankIn {
        BankIn {
            title: "fixture discovery".into(),
            when: "fixture condition".into(),
            found: "fixture result".into(),
            how: "fixture measurement".into(),
            reconsider: String::new(),
            evidence: "verified! (caller text, not a verification grant)".into(),
            tags: String::new(),
            project: String::new(),
            persona: String::new(),
            applicability_harness: String::new(),
            applicability_version: String::new(),
        }
    }
    fn fixture() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "instinct-bank-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&p).unwrap();
        p.join("bank.jsonl")
    }
    #[test]
    fn lexical_score_preserves_lengths_case_and_repeated_terms() {
        let documents = vec!["Čas CLOCK clock".into(), "čas other".into(), "a".into()];
        let scores = bm25("CLOCK čas clock", &documents);
        let weight = |df: f32| ((3.0 - df + 0.5) / (df + 0.5) + 1.0).ln();
        let term = |df: f32, tf: f32, len: f32| {
            weight(df) * tf * 2.2 / (tf + 1.2 * (0.25 + 0.75 * len / (5.0 / 3.0)))
        };
        assert_eq!(
            scores,
            vec![
                term(1.0, 2.0, 3.0) + term(2.0, 1.0, 3.0),
                term(2.0, 1.0, 2.0),
                0.0
            ]
        );
        assert_eq!(bm25("", &documents), vec![0.0; 3]);
    }

    #[test]
    fn decision_context_transfers_a_method_with_its_boundary() {
        let path = fixture();
        let mut entry = bank_input();
        entry.when = "command acknowledgement delay".into();
        entry.how = "Compare command and acknowledgement timestamps".into();
        entry.reconsider = "If clocks differ, measure the offset first".into();
        let d = bank_discovery(entry, &eligibility::Scope::default()).unwrap();
        append(&path, &d.to_line()).unwrap();
        let mut private = bank_input();
        private.when = d.when.clone();
        private.project = "other-project".into();
        let restricted = bank_discovery(private, &eligibility::Scope::default()).unwrap();
        append(&path, &restricted.to_line()).unwrap();
        let scope = eligibility::Scope::default();
        let query = "New controller has an acknowledgement delay";
        for hits in 0..=1 {
            let data = parse_json(&decision_context(&path, query, &scope).unwrap()).unwrap();
            let Some(Json::Arr(methods)) = data.get("experiences") else {
                panic!("missing experiences")
            };
            assert_eq!(methods.len(), 1);
            assert_eq!(
                methods[0].get("method").and_then(Json::as_str),
                Some(d.how.as_str())
            );
            assert_eq!(
                methods[0].get("reconsider_if").and_then(Json::as_str),
                Some(d.reconsider.as_str())
            );
            assert_eq!(
                methods[0].get("hits").and_then(Json::as_f64),
                Some(hits as f64)
            );
            if hits == 0 {
                op_mark(
                    &path,
                    "hit",
                    &d.id,
                    "Independent timing check succeeded",
                    "",
                )
                .unwrap();
            }
        }
        let empty = decision_context(&path, "colour palette", &scope).unwrap();
        assert!(
            matches!(parse_json(&empty).unwrap().get("experiences"),Some(Json::Arr(a)) if a.is_empty())
        );
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn offline_experience_lifecycle() {
        let path = fixture();
        let mut entry = bank_input();
        entry.when = "signal comparison uses a proxy whose score may disagree with independent task outcomes".into();
        entry.found = "Similarity can reward responses that never accomplished the task".into();
        entry.how = "Compare each proxy score against an independent completion check".into();
        entry.evidence =
            "Synthetic sample: two high-similarity failures and one low-similarity success".into();
        let d = bank_discovery(entry, &eligibility::Scope::default()).unwrap();
        append(&path, &d.to_line()).unwrap();
        let query = "Which proxy score tracks completion of a translation task?";
        let read = || op_recall_scoped(&path, query, 1, &eligibility::Scope::default()).unwrap();
        assert!(read().contains(&d.how));
        assert!(op_recall(&path, "prepare dinner", 1).unwrap().is_empty());
        assert!(op_instincts(&path, 2).unwrap().is_empty());
        for case in [
            "translation: independent check passed",
            "retrieval: independent check passed",
        ] {
            op_mark(&path, "hit", &d.id, case, "").unwrap();
        }
        assert!(op_instincts(&path, 2).unwrap().contains(&d.id));
        op_review(
            &path,
            &d.id,
            "quarantined",
            "Changed applicability",
            "fixture:v2",
        )
        .unwrap();
        assert!(read().is_empty());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn scope_and_quarantine_survive_roundtrip_and_append() {
        let p = fixture();
        let original="{\"kind\":\"discovery\",\"id\":\"in-fixture\",\"title\":\"test\",\"scope_persona\":\"koscak\",\"source\":\"mined\",\"state\":\"candidate\"}\n";
        fs::write(&p, original).unwrap();
        let d = load(&p).unwrap().remove(0);
        let v = parse_json(&d.to_line()).unwrap();
        assert_eq!(
            v.get("scope_persona").and_then(Json::as_str),
            Some("koscak")
        );
        op_review(
            &p,
            "in-fixture",
            "quarantined",
            "contradictory source",
            "fixture:1",
        )
        .unwrap();
        assert!(fs::read_to_string(&p).unwrap().starts_with(original));
        let d = load(&p).unwrap().remove(0);
        assert_eq!(
            eligibility::scope_check(&d.meta, &eligibility::Scope::default(), 1),
            Err("quarantined")
        );
    }
    #[test]
    fn receipt_has_exact_ids_and_unknown_session_is_explicit() {
        let r = recall_receipt(&["in-one".into(), "in-two".into()], None).unwrap();
        assert!(matches!(r.get("ids"),Some(Json::Arr(v)) if v.len()==2));
        assert!(!r.get("event_id").and_then(Json::as_str).unwrap().is_empty());
        assert!(recall_receipt(&[], None).is_none());
    }
    #[test]
    fn review_never_grants_verified_status() {
        let p = fixture();
        assert!(op_review(&p, "x", "validated", "because", "ref").is_err());
        assert!(!p.exists());
    }
    #[test]
    fn scope_missing_cannot_match_explicit_requirement() {
        let m = meta_from(
            &parse_json("{\"scope_project\":\"/project/a\",\"scope_harness\":\"codex\"}").unwrap(),
        );
        assert_eq!(
            eligibility::scope_check(&m, &eligibility::Scope::default(), 1),
            Err("project")
        );
    }
    #[test]
    fn bank_creation_persists_explicit_scope_without_verification_escalation() {
        let mut b = bank_input();
        b.project = "/project/a".into();
        b.persona = "persona-a".into();
        b.applicability_harness = "codex".into();
        b.applicability_version = "version-a".into();
        let d = bank_discovery(b, &eligibility::Scope::default()).unwrap();
        let p = fixture();
        append(&p, &d.to_line()).unwrap();
        let stored = load(&p).unwrap().remove(0);
        assert_eq!(
            scope_identity(&stored.meta),
            ["/project/a", "persona-a", "codex", "version-a"]
        );
        assert_eq!(stored.meta.source, "banked");
        assert_eq!(stored.meta.state, "candidate");
        assert_eq!(stored.meta.evidence_status, "unverified");
        assert!(eligibility::unverified(&stored.meta));
        assert_eq!(
            eligibility::scope_check(&stored.meta, &eligibility::Scope::default(), 1),
            Err("project")
        );
    }
    #[test]
    fn bank_creation_does_not_infer_applicability_from_receipt_origin() {
        let origin = eligibility::Scope {
            project: "/explicit/project".into(),
            persona: "explicit-persona".into(),
            harness: "claude".into(),
            version: "origin-version".into(),
        };
        let d = bank_discovery(bank_input(), &origin).unwrap();
        assert_eq!(
            scope_identity(&d.meta),
            ["/explicit/project", "explicit-persona", "", ""]
        );
        assert!(bank_scope_note(&d.meta).contains("applicability harness, applicability version"));
        let d = bank_discovery(bank_input(), &eligibility::Scope::default()).unwrap();
        assert_eq!(d.id, make_id(&d.title, &d.found));
        assert!(bank_scope_note(&d.meta)
            .contains("project, persona, applicability harness, applicability version"));
        assert_eq!(
            eligibility::eligible(&d.meta, &eligibility::Scope::default(), 1, None, 0, false),
            Err("candidate_match_unproven")
        );
    }
    #[test]
    fn bank_creation_keeps_each_scope_identity_distinct() {
        let mut ids = std::collections::HashSet::new();
        let p = fixture();
        for scope in [
            ["p1", "a", "codex", "v1"],
            ["p2", "a", "codex", "v1"],
            ["p1", "b", "codex", "v1"],
            ["p1", "a", "claude", "v1"],
            ["p1", "a", "codex", "v2"],
        ] {
            let mut b = bank_input();
            b.project = scope[0].into();
            b.persona = scope[1].into();
            b.applicability_harness = scope[2].into();
            b.applicability_version = scope[3].into();
            let d = bank_discovery(b, &eligibility::Scope::default()).unwrap();
            assert!(ids.insert(d.id.clone()));
            append(&p, &d.to_line()).unwrap();
        }
        let all = load(&p).unwrap();
        assert_eq!(all.len(), 5);
        let scope = eligibility::Scope {
            project: "p1".into(),
            persona: "a".into(),
            harness: "codex".into(),
            version: "v1".into(),
        };
        assert_eq!(
            all.iter()
                .filter(|d| eligibility::eligible(&d.meta, &scope, 1, Some(0.99), 0, false).is_ok())
                .count(),
            1
        );
        assert_eq!(
            all.iter()
                .filter(|d| eligibility::eligible(
                    &d.meta,
                    &eligibility::Scope::default(),
                    1,
                    Some(0.99),
                    0,
                    false
                )
                .is_ok())
                .count(),
            0
        );
    }
}

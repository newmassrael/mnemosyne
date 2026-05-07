//! Phase -1A stage 2A XTDB baseline client (DESIGN.md §18 line 1819-1830).
//!
//! Talks to a JUXT XTDB 1.x standalone server over HTTP. We host the server
//! out-of-process per Priority 0 (d) decision (subprocess RPC; JNI rejected
//! for JVM heap leak / measurement noise reasons).
//!
//! This crate measures the *non-branching* subset of §3 SLAs only — XTDB
//! offers no native branching primitive (§18 line 1829), so the branching
//! SLAs are owned by the direct-impl crate and any cross-stage diff lives in
//! the §18 decision gate, not in this measurement run.

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const DEFAULT_URL: &str = "http://127.0.0.1:3000";

#[derive(Clone, Debug)]
pub struct XtdbClient {
 base_url: String,
 http: Client,
}

impl XtdbClient {
 pub fn new(base_url: impl Into<String>) -> Result<Self> {
 let http = Client::builder()
 .timeout(Duration::from_secs(60))
 .pool_idle_timeout(Duration::from_secs(60))
 .build()
 .context("build reqwest client")?;
 Ok(Self {
 base_url: base_url.into(),
 http,
 })
 }

 pub async fn status(&self) -> Result<Value> {
 let url = format!("{}/_xtdb/status", self.base_url);
 let resp = self
 .http
 .get(&url)
 .header("Accept", "application/json")
 .send()
 .await
 .with_context(|| format!("GET {}", url))?
 .error_for_status()?;
 let v: Value = resp.json().await.context("decode status JSON")?;
 Ok(v)
 }

 /// Submit a single transaction containing one or more put operations.
 /// Returns the resolved tx-id (`tx-id` integer in v1.x).
 pub async fn submit_tx_puts(&self, docs: &[Value]) -> Result<i64> {
 let mut tx_ops: Vec<Value> = Vec::with_capacity(docs.len());
 for d in docs {
 tx_ops.push(json!(["put", d]));
 }
 let url = format!("{}/_xtdb/submit-tx", self.base_url);
 let body = json!({ "tx-ops": tx_ops });
 let resp = self
 .http
 .post(&url)
 .header("Content-Type", "application/json")
 .header("Accept", "application/json")
 .body(serde_json::to_string(&body)?)
 .send()
 .await
 .with_context(|| format!("POST {}", url))?;
 let status = resp.status();
 let text = resp.text().await.unwrap_or_default();
 if !status.is_success() {
 return Err(anyhow!("submit-tx failed: {} body={}", status, text));
 }
 let v: Value = serde_json::from_str(&text)
 .with_context(|| format!("decode submit-tx JSON: {}", text))?;
 let tx_id = v
 .get("txId")
 .or_else(|| v.get("tx-id"))
 .and_then(|x| x.as_i64())
 .ok_or_else(|| anyhow!("submit-tx response missing tx-id: {}", v))?;
 Ok(tx_id)
 }

 /// Wait for the server to materialise transaction `tx_id` into the index.
 pub async fn await_tx(&self, tx_id: i64) -> Result<()> {
 let url = format!("{}/_xtdb/await-tx?txId={}", self.base_url, tx_id);
 let resp = self
 .http
 .get(&url)
 .header("Accept", "application/json")
 .send()
 .await
 .with_context(|| format!("GET {}", url))?;
 if !resp.status().is_success() {
 let s = resp.status();
 let t = resp.text().await.unwrap_or_default();
 return Err(anyhow!("await-tx failed: {} body={}", s, t));
 }
 Ok(())
 }

 /// Synchronous entity-by-id read.
 pub async fn get_entity(&self, eid: &str) -> Result<Option<Value>> {
 // eid must be EDN-formatted in the query string; for string ids that
 // means surrounding double quotes.
 let edn = format!("\"{}\"", eid);
 let url = format!(
 "{}/_xtdb/entity?eid-edn={}",
 self.base_url,
 urlencode(&edn)
 );
 let resp = self
 .http
 .get(&url)
 .header("Accept", "application/json")
 .send()
 .await
 .with_context(|| format!("GET {}", url))?;
 if resp.status() == reqwest::StatusCode::NOT_FOUND {
 return Ok(None);
 }
 if !resp.status().is_success() {
 let s = resp.status();
 let t = resp.text().await.unwrap_or_default();
 return Err(anyhow!("get_entity failed: {} body={}", s, t));
 }
 let body = resp.text().await?;
 if body.trim().is_empty() || body.trim() == "null" {
 return Ok(None);
 }
 Ok(Some(serde_json::from_str(&body)?))
 }

 /// Datalog query (passed as an EDN string). Returns the rows array as
 /// returned by the server.
 pub async fn query_edn(&self, edn_query: &str) -> Result<Vec<Value>> {
 let url = format!("{}/_xtdb/query", self.base_url);
 let resp = self
 .http
 .post(&url)
 .header("Content-Type", "application/edn")
 .header("Accept", "application/json")
 .body(format!("{{:query {} }}", edn_query))
 .send()
 .await
 .with_context(|| format!("POST {}", url))?;
 if !resp.status().is_success() {
 let s = resp.status();
 let t = resp.text().await.unwrap_or_default();
 return Err(anyhow!("query failed: {} body={}", s, t));
 }
 let body = resp.text().await?;
 let v: Value = serde_json::from_str(&body).with_context(|| body.clone())?;
 match v {
 Value::Array(rows) => Ok(rows),
 other => Err(anyhow!("unexpected query response shape: {}", other)),
 }
 }
}

fn urlencode(s: &str) -> String {
 let mut out = String::with_capacity(s.len() * 3);
 for b in s.bytes() {
 let c = b as char;
 if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
 out.push(c);
 } else {
 out.push_str(&format!("%{:02X}", b));
 }
 }
 out
}

// ─── Workload → XTDB document mapping ────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntityDoc<'a> {
 #[serde(rename = "xt/id")]
 pub id: String,
 pub kind: u8,
 pub name: &'a str,
}

pub fn entity_doc(id: u64, kind: u8, name: &str) -> Value {
 json!({
 "xt/id": format!("e:{}", id),
 "kind": kind,
 "name": name,
 })
}

pub fn fact_doc(id: u64, predicate: &str, subject: u64, object: u64) -> Value {
 json!({
 "xt/id": format!("f:{}", id),
 "predicate": predicate,
 "subject": format!("e:{}", subject),
 "object": format!("e:{}", object),
 })
}

/// Person entity with the minimum T2 fields needed for the family-tree /
/// birth-year invariant used by the T1+T2 measurement: `birth_year` + an
/// optional `parent` reference. The id space overlaps with `entity_doc`
/// (`p:N` vs `e:N`) so the T2 spike can be populated alongside the
/// non-branching baseline without colliding with the existing entity ids.
pub fn person_doc(id: u64, name: &str, birth_year: i64, parent: Option<u64>) -> Value {
 let mut v = json!({
 "xt/id": format!("p:{}", id),
 "name": name,
 "birth_year": birth_year,
 });
 if let Some(par) = parent {
 v["parent"] = Value::String(format!("p:{}", par));
 }
 v
}

/// Agent document carrying an explicit `known` set of fact ids — the
/// Epistemic-CF analogue used by the §15 / §11 measurement (DESIGN.md §11
/// epistemic CF, §18 line 1928). One agent + N known fact ids per doc.
pub fn agent_doc(agent_id: u64, name: &str, known: &[u64]) -> Value {
 let known_strs: Vec<String> = known.iter().map(|f| format!("f:{}", f)).collect();
 json!({
 "xt/id": format!("a:{}", agent_id),
 "name": name,
 "known": known_strs,
 })
}

/// Asset document — content_hash blob + branch_id + an inlined
/// `facts_referenced` list. The list is encoded inline (blob path) rather
/// than normalised to row-per-(asset, fact); the §18 storage-growth
/// measurement only needs to compare *order of magnitude* on disk between
/// the XTDB and direct-impl representations, and inlined lists give the
/// minimum possible XTDB representation for that comparison. (The §4
/// normalised decision is owned by the direct-impl prototype and the
/// schema-bench micro-bench, not by this measurement.)
pub fn asset_doc(asset_id: u64, content_hash: &[u8; 32], facts_referenced: &[u64], branch_id: u64) -> Value {
 let facts: Vec<String> = facts_referenced
 .iter()
 .map(|f| format!("f:{}", f))
 .collect();
 json!({
 "xt/id": format!("as:{}", asset_id),
 "content_hash": hex_lower(content_hash),
 "facts_referenced": facts,
 "branch_id": branch_id,
 })
}

fn hex_lower(b: &[u8]) -> String {
 let mut s = String::with_capacity(b.len() * 2);
 for byte in b {
 s.push_str(&format!("{:02x}", byte));
 }
 s
}

#[cfg(test)]
mod tests {
 use super::*;

 #[test]
 fn entity_doc_shape() {
 let d = entity_doc(7, 1, "alice");
 assert_eq!(d["xt/id"], json!("e:7"));
 assert_eq!(d["kind"], json!(1));
 assert_eq!(d["name"], json!("alice"));
 }

 #[test]
 fn fact_doc_shape() {
 let d = fact_doc(42, "knows", 1, 2);
 assert_eq!(d["xt/id"], json!("f:42"));
 assert_eq!(d["subject"], json!("e:1"));
 assert_eq!(d["object"], json!("e:2"));
 }

 #[test]
 fn person_doc_shape() {
 let p = person_doc(3, "carol", 1990, Some(1));
 assert_eq!(p["xt/id"], json!("p:3"));
 assert_eq!(p["birth_year"], json!(1990));
 assert_eq!(p["parent"], json!("p:1"));
 let orphan = person_doc(1, "root", 1900, None);
 assert!(orphan.get("parent").is_none());
 }

 #[test]
 fn agent_doc_shape() {
 let a = agent_doc(2, "bob", &[10, 11]);
 assert_eq!(a["xt/id"], json!("a:2"));
 assert_eq!(a["known"], json!(["f:10", "f:11"]));
 }

 #[test]
 fn url_encode_basic() {
 assert_eq!(urlencode("\"e:1\""), "%22e%3A1%22");
 }
}

//! Phase -1A stage 2A XTDB baseline measurement runner (DESIGN.md §18).
//!
//! Subcommands:
//! - `wait` — block until the server's `/status` endpoint is ready.
//! - `populate` — bulk-insert a workload (entities + facts) into XTDB.
//! - `bench` — run the non-branching SLA measurements (point query + 3-hop).

use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use hdrhistogram::Histogram;
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use serde_json::Value;
use workload_gen::{generate, Workload};
use xtdb_baseline::{
 agent_doc, entity_doc, fact_doc, person_doc, XtdbClient, DEFAULT_URL,
};

#[derive(Parser)]
#[command(name = "xtdb-bench", version, about = "Phase -1A stage 2A XTDB baseline runner")]
struct Cli {
 #[arg(long, default_value = DEFAULT_URL)]
 url: String,
 #[command(subcommand)]
 cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
 Wait {
 #[arg(long, default_value_t = 60)]
 timeout_secs: u64,
 },
 Populate {
 #[arg(long, default_value_t = 5_000)]
 entities: usize,
 #[arg(long, default_value_t = 2_000)]
 facts: usize,
 #[arg(long, default_value_t = 500)]
 batch_size: usize,
 },
 Bench {
 #[arg(long, default_value_t = 5_000)]
 entities: usize,
 #[arg(long, default_value_t = 2_000)]
 facts: usize,
 #[arg(long, default_value_t = 500)]
 batch_size: usize,
 #[arg(long, default_value_t = 1_000)]
 samples: usize,
 },
 /// §18 line 1924-1948: remaining 5 SLA measurements. T1+T2 / epistemic / closure /
 /// indexing-lag (SecondaryDB proxy) — measured by populating 5K entries above a single iteration.
 BenchRemaining {
 #[arg(long, default_value_t = 5_000)]
 entities: usize,
 #[arg(long, default_value_t = 2_000)]
 facts: usize,
 #[arg(long, default_value_t = 1_000)]
 people: usize,
 #[arg(long, default_value_t = 100)]
 agents: usize,
 #[arg(long, default_value_t = 500)]
 batch_size: usize,
 #[arg(long, default_value_t = 1_000)]
 samples: usize,
 },
 /// §18 line 1944 storage growth — populate the full default 200K-asset
 /// workload (entities + facts + assets, all CFs in scope) and then have
 /// the operator read the on-disk size with `du`.
 StorageGrowth {
 #[arg(long, default_value_t = 1_000)]
 batch_size: usize,
 },
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
 tracing_subscriber::fmt()
 .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
 .with_target(false)
 .compact()
 .init();
 let cli = Cli::parse();
 let client = XtdbClient::new(&cli.url)?;
 match cli.cmd {
 Cmd::Wait { timeout_secs } => wait(&client, timeout_secs).await,
 Cmd::Populate {
 entities,
 facts,
 batch_size,
 } => {
 let workload = small_workload(entities, facts);
 populate(&client, &workload, batch_size).await
 }
 Cmd::Bench {
 entities,
 facts,
 batch_size,
 samples,
 } => {
 let workload = small_workload(entities, facts);
 populate(&client, &workload, batch_size).await?;
 bench(&client, &workload, samples).await
 }
 Cmd::BenchRemaining {
 entities,
 facts,
 people,
 agents,
 batch_size,
 samples,
 } => {
 let workload = small_workload(entities, facts);
 populate(&client, &workload, batch_size).await?;
 populate_t2(&client, people, agents, &workload, batch_size).await?;
 bench_remaining(&client, &workload, people, agents, samples).await
 }
 Cmd::StorageGrowth { batch_size } => {
 // Default workload: 200K assets, 50K facts, 1K branches, 1K
 // agents, 16,630 entities (workload-gen default config). All four
 // doc kinds (entity / fact / asset / agent) are populated so the
 // measured du covers the full §18 line 1944 storage footprint.
 let workload = generate(&workload_gen::default_config());
 populate(&client, &workload, batch_size).await?;
 populate_assets(&client, &workload, batch_size).await?;
 println!(
  "STORAGE_GROWTH_MARKER entities={} facts={} assets={} populated_ok=true",
  workload.entities.len(),
  workload.facts.len(),
  workload.assets.len()
 );
 Ok(())
 }
 }
}

async fn populate_assets(
 client: &XtdbClient,
 workload: &Workload,
 batch_size: usize,
) -> Result<()> {
 use xtdb_baseline::asset_doc;
 println!("populate_assets: {} assets, batch={}", workload.assets.len(), batch_size);
 let t0 = Instant::now();
 let mut last_tx: Option<i64> = None;
 let mut buf: Vec<Value> = Vec::with_capacity(batch_size);
 for a in &workload.assets {
 buf.push(asset_doc(a.id, &a.content_hash, &a.facts_referenced, a.branch_id));
 if buf.len() >= batch_size {
 let tx = client.submit_tx_puts(&buf).await?;
 last_tx = Some(tx);
 buf.clear();
 }
 }
 if !buf.is_empty() {
 let tx = client.submit_tx_puts(&buf).await?;
 last_tx = Some(tx);
 }
 if let Some(tx) = last_tx {
 client.await_tx(tx).await.context("await final asset tx")?;
 }
 let elapsed = t0.elapsed();
 println!(
 "populate_assets done in {:?} ({:.0} doc/s)",
 elapsed,
 workload.assets.len() as f64 / elapsed.as_secs_f64()
 );
 Ok(())
}

fn small_workload(entities: usize, facts: usize) -> Workload {
 let mut cfg = workload_gen::default_config();
 cfg.assets = 0; // assets unused for non-branching baseline
 cfg.facts = facts;
 cfg.branches = 1;
 cfg.agents = 0;
 let total_entity_target = entities;
 cfg.entity_dist = workload_gen::EntityDistribution {
 person: total_entity_target / 4,
 place: total_entity_target / 16,
 faction: total_entity_target / 64,
 event: total_entity_target / 4,
 item: total_entity_target / 4,
 concept: total_entity_target
 - (total_entity_target / 4
  + total_entity_target / 16
  + total_entity_target / 64
  + total_entity_target / 4
  + total_entity_target / 4),
 };
 generate(&cfg)
}

async fn wait(client: &XtdbClient, timeout_secs: u64) -> Result<()> {
 let deadline = Instant::now() + Duration::from_secs(timeout_secs);
 let mut last_err = String::new();
 while Instant::now() < deadline {
 match client.status().await {
 Ok(v) => {
  println!("xtdb up: {}", v);
  return Ok(());
 }
 Err(e) => {
  last_err = e.to_string();
  tokio::time::sleep(Duration::from_millis(500)).await;
 }
 }
 }
 bail!("xtdb did not respond within {}s — last error: {}", timeout_secs, last_err);
}

async fn populate(client: &XtdbClient, workload: &Workload, batch_size: usize) -> Result<()> {
 println!(
 "populate: {} entities, {} facts, batch={}",
 workload.entities.len(),
 workload.facts.len(),
 batch_size
 );
 let t0 = Instant::now();

 let mut last_tx: Option<i64> = None;
 let mut buf: Vec<Value> = Vec::with_capacity(batch_size);
 for e in &workload.entities {
 buf.push(entity_doc(e.id, e.kind as u8, &e.name));
 if buf.len() >= batch_size {
 let tx = client.submit_tx_puts(&buf).await?;
 last_tx = Some(tx);
 buf.clear();
 }
 }
 if !buf.is_empty() {
 let tx = client.submit_tx_puts(&buf).await?;
 last_tx = Some(tx);
 buf.clear();
 }
 for f in &workload.facts {
 buf.push(fact_doc(f.id, &f.predicate, f.subject, f.object));
 if buf.len() >= batch_size {
 let tx = client.submit_tx_puts(&buf).await?;
 last_tx = Some(tx);
 buf.clear();
 }
 }
 if !buf.is_empty() {
 let tx = client.submit_tx_puts(&buf).await?;
 last_tx = Some(tx);
 }
 if let Some(tx) = last_tx {
 client.await_tx(tx).await.context("await final tx")?;
 }
 let elapsed = t0.elapsed();
 println!(
 "populate done in {:?} ({:.0} doc/s)",
 elapsed,
 (workload.entities.len() + workload.facts.len()) as f64 / elapsed.as_secs_f64()
 );
 Ok(())
}

async fn bench(client: &XtdbClient, workload: &Workload, samples: usize) -> Result<()> {
 let entity_ids: Vec<u64> = workload.entities.iter().map(|e| e.id).collect();
 let mut rng = ChaCha20Rng::seed_from_u64(0x7B_07_DB_07_DB_07_DB_07);

 // ── 1) Asset save (T1) p95 — single put + await ──
 let mut t1_save = Histogram::<u64>::new(3).unwrap();
 let put_iters = samples.min(500); // each iter does a synchronous tx-await round-trip
 let mut next_synth_id: u64 = 9_000_000;
 for _ in 0..put_iters {
 let id = next_synth_id;
 next_synth_id += 1;
 let docs = vec![entity_doc(id, 1, "synth")];
 let t = Instant::now();
 let tx = client.submit_tx_puts(&docs).await?;
 client.await_tx(tx).await?;
 t1_save.record(t.elapsed().as_nanos() as u64).ok();
 }
 print_hist("xtdb_t1_save (submit+await)", &t1_save, "<50ms");

 // ── 2) Point query — entity by eid ──
 let mut point = Histogram::<u64>::new(3).unwrap();
 for _ in 0..samples {
 let eid_u = entity_ids[rng.gen_range(0..entity_ids.len())];
 let eid = format!("e:{}", eid_u);
 let t = Instant::now();
 let _ = client.get_entity(&eid).await?;
 point.record(t.elapsed().as_nanos() as u64).ok();
 }
 print_hist("xtdb_point_query", &point, "<50ms");

 // ── 3) 3-hop graph traversal via Datalog ──
 let mut three_hop = Histogram::<u64>::new(3).unwrap();
 let three_hop_iters = samples.min(200);
 for _ in 0..three_hop_iters {
 let eid_u = entity_ids[rng.gen_range(0..entity_ids.len())];
 // {:find [?o3]
 // :in [?start]
 // :where [[?f1 :subject ?start] [?f1 :object ?o1]
 //  [?f2 :subject ?o1] [?f2 :object ?o2]
 //  [?f3 :subject ?o2] [?f3 :object ?o3]]}
 let edn = format!(
 "{{:find [?o3] :in [?start] :where [[?f1 :subject ?start] [?f1 :object ?o1] [?f2 :subject ?o1] [?f2 :object ?o2] [?f3 :subject ?o2] [?f3 :object ?o3]] :limit 100}} :in-args [\"e:{}\"]",
 eid_u
 );
 let _ = edn; // legacy single-arg path below uses simpler form
 let edn_simple = format!(
 "{{:find [?o3] :where [[?f1 :subject \"e:{}\"] [?f1 :object ?o1] [?f2 :subject ?o1] [?f2 :object ?o2] [?f3 :subject ?o2] [?f3 :object ?o3]] :limit 100}}",
 eid_u
 );
 let t = Instant::now();
 let _ = client.query_edn(&edn_simple).await?;
 three_hop.record(t.elapsed().as_nanos() as u64).ok();
 }
 print_hist("xtdb_three_hop", &three_hop, "<100ms");

 println!("\n--- §18 line 1924-1948 non-branching subset (XTDB baseline) ---");
 println!(
 "T1 save p95 {:>10} target < 50ms",
 fmt_ns(t1_save.value_at_quantile(0.95))
 );
 println!(
 "point p95 {:>10} target < 50ms",
 fmt_ns(point.value_at_quantile(0.95))
 );
 println!(
 "3-hop p95 {:>10} target < 100ms",
 fmt_ns(three_hop.value_at_quantile(0.95))
 );
 Ok(())
}

fn print_hist(label: &str, hist: &Histogram<u64>, target: &str) {
 if hist.is_empty() {
 println!("{}: <no samples>", label);
 return;
 }
 println!(
 "{:30} n={:>5} p50 {:>10} p95 {:>10} p99 {:>10} max {:>10} target {}",
 label,
 hist.len(),
 fmt_ns(hist.value_at_quantile(0.50)),
 fmt_ns(hist.value_at_quantile(0.95)),
 fmt_ns(hist.value_at_quantile(0.99)),
 fmt_ns(hist.max()),
 target
 );
}

fn fmt_ns(ns: u64) -> String {
 if ns < 10_000 {
 format!("{} ns", ns)
 } else if ns < 10_000_000 {
 format!("{:.1} µs", ns as f64 / 1_000.0)
 } else if ns < 10_000_000_000 {
 format!("{:.2} ms", ns as f64 / 1_000_000.0)
 } else {
 format!("{:.2} s", ns as f64 / 1_000_000_000.0)
 }
}

// ── Phase -1A stage 2A remaining 5 SLA — T2 / epistemic / closure / lag ────────────

/// Populate the family-tree fixture (T2 invariant target) and the agent
/// fixture (epistemic target) on top of the existing entity / fact baseline.
/// Family tree: linear `people` count, child[i].parent = people[i-1] for i>0.
/// Agents: each agent knows a deterministic 10-fact slice of the workload's
/// fact ids — small enough that the doc fits in a single round-trip.
async fn populate_t2(
 client: &XtdbClient,
 people: usize,
 agents: usize,
 workload: &Workload,
 batch_size: usize,
) -> Result<()> {
 println!(
 "populate_t2: {} people (linear family), {} agents",
 people, agents
 );
 let t0 = Instant::now();
 let mut last_tx: Option<i64> = None;
 let mut buf: Vec<Value> = Vec::with_capacity(batch_size);
 for i in 0..people {
 let id = i as u64 + 1;
 let parent = if i == 0 { None } else { Some(id - 1) };
 // Ascending birth_year so the invariant `child.birth_year >
 // parent.birth_year` is *satisfied* by construction.
 let birth_year = 1900 + i as i64;
 buf.push(person_doc(id, &format!("p{}", id), birth_year, parent));
 if buf.len() >= batch_size {
 let tx = client.submit_tx_puts(&buf).await?;
 last_tx = Some(tx);
 buf.clear();
 }
 }
 if !buf.is_empty() {
 let tx = client.submit_tx_puts(&buf).await?;
 last_tx = Some(tx);
 buf.clear();
 }

 let fact_ids: Vec<u64> = workload.facts.iter().map(|f| f.id).collect();
 if fact_ids.is_empty() && agents > 0 {
 bail!("agents requested but workload has no facts");
 }
 for i in 0..agents {
 let agent_id = i as u64 + 1;
 // Each agent knows a deterministic 10-fact slice (wraps around).
 let known: Vec<u64> = (0..10)
 .map(|k| fact_ids[(i * 13 + k) % fact_ids.len()])
 .collect();
 buf.push(agent_doc(agent_id, &format!("a{}", agent_id), &known));
 if buf.len() >= batch_size {
 let tx = client.submit_tx_puts(&buf).await?;
 last_tx = Some(tx);
 buf.clear();
 }
 }
 if !buf.is_empty() {
 let tx = client.submit_tx_puts(&buf).await?;
 last_tx = Some(tx);
 }
 if let Some(tx) = last_tx {
 client.await_tx(tx).await.context("await final t2 tx")?;
 }
 let elapsed = t0.elapsed();
 println!(
 "populate_t2 done in {:?} ({} people + {} agents)",
 elapsed, people, agents
 );
 Ok(())
}

async fn bench_remaining(
 client: &XtdbClient,
 workload: &Workload,
 people: usize,
 agents: usize,
 samples: usize,
) -> Result<()> {
 if people < 2 {
 bail!("need at least 2 people for T2 invariant measurement");
 }

 // ── 1) T1+T2 save p95 — single put + await + invariant query ──
 //
 // The invariant query `child.birth_year > parent.birth_year` is run
 // immediately after the put resolves; the wall clock measurement covers
 // the full round-trip (submit + await + validation query response). Each
 // iteration inserts a *fresh* descendant so the tx is non-trivial.
 let mut t1t2 = Histogram::<u64>::new(3).unwrap();
 let t1t2_iters = samples.min(500);
 let mut next_pid: u64 = (people as u64) + 1;
 let last_existing = (people as u64).saturating_sub(1).max(1);
 for _ in 0..t1t2_iters {
 let pid = next_pid;
 next_pid += 1;
 let parent = last_existing; // every new person hangs off the last seeded ancestor
 let birth_year = 1900 + people as i64 + (pid as i64);
 let doc = person_doc(pid, &format!("p{}", pid), birth_year, Some(parent));
 let t = Instant::now();
 let tx = client.submit_tx_puts(&[doc]).await?;
 client.await_tx(tx).await?;
 let edn = format!(
 "{{:find [?c] :where [[?c :xt/id \"p:{}\"] [?c :birth_year ?cy] [?c :parent ?par] [?par :birth_year ?py] [(<= ?cy ?py)]] :limit 1}}",
 pid
 );
 let violations = client.query_edn(&edn).await?;
 if !violations.is_empty() {
 bail!("T2 invariant violated for p{} — should not happen with monotonic seeding", pid);
 }
 t1t2.record(t.elapsed().as_nanos() as u64).ok();
 }
 print_hist("xtdb_t1_t2_save", &t1t2, "<100ms");

 // ── 2) Epistemic single-agent p95 ──
 //
 // Pull `:known` for one agent. Datalog with `(pull ?a [:known])`. We
 // measure the round-trip including the query parse + result serialize.
 let mut epistemic = Histogram::<u64>::new(3).unwrap();
 let mut rng = ChaCha20Rng::seed_from_u64(0x_E915_7E30_E915_7E30);
 let _ = &mut rng; // future expansion; current loop is deterministic by index
 if agents == 0 {
 println!("xtdb_epistemic_single_agent: skipped (no agents seeded)");
 } else {
 for i in 0..samples {
 let agent_id = (i % agents) as u64 + 1;
 let edn = format!(
  "{{:find [(pull ?a [:known])] :where [[?a :xt/id \"a:{}\"]] :limit 1}}",
  agent_id
 );
 let t = Instant::now();
 let _ = client.query_edn(&edn).await?;
 epistemic.record(t.elapsed().as_nanos() as u64).ok();
 }
 print_hist("xtdb_epistemic_single_agent", &epistemic, "<50ms");
 }

 // ── 3) Bounded closure (k=2) validation ──
 //
 // §18 line 1934 specifies *bounded closure (k=2) full validation*: one
 // pass that materialises every (descendant, ancestor) pair reachable in
 // ≤ 2 parent hops from each person. With a linear N-tree this yields
 // ~2N rows (parent + grandparent for each node), O(N) total — distinct
 // from the open-ended `ancestor` recursion which would explode to N²
 // rows on a chain. We measure both the k=1 (parent) and k=2
 // (grandparent) leaves separately so the bounded total is unambiguous.
 let edn_k1 = "{:find [?d ?a] :where [[?d :parent ?a]] :limit 200000}".to_string();
 let edn_k2 = "{:find [?d ?gp] :where [[?d :parent ?p] [?p :parent ?gp]] :limit 200000}"
 .to_string();
 let closure_iters = 5usize;
 let mut closure_hist = Histogram::<u64>::new(3).unwrap();
 let mut k1_rows: usize = 0;
 let mut k2_rows: usize = 0;
 for _ in 0..closure_iters {
 let t = Instant::now();
 let r1 = client.query_edn(&edn_k1).await?;
 let r2 = client.query_edn(&edn_k2).await?;
 let elapsed = t.elapsed();
 closure_hist.record(elapsed.as_nanos() as u64).ok();
 k1_rows = r1.len();
 k2_rows = r2.len();
 }
 let closure_p50 = closure_hist.value_at_quantile(0.50);
 println!(
 "xtdb_closure_k2_bounded n={} p50 {} max {} k1_rows={} k2_rows={} target <1.25min @50K",
 closure_iters,
 fmt_ns(closure_p50),
 fmt_ns(closure_hist.max()),
 k1_rows,
 k2_rows
 );
 // Bounded closure scales O(N) in pairs, so the linear projection to 50K
 // is `closure_p50 * (50_000 / people)`. Reported alongside the observed
 // value.
 let scale = 50_000f64 / people as f64;
 let projected_ns = closure_p50 as f64 * scale;
 println!(
 " → 50K linear projection (O(N), factor {:.0}×): {}",
 scale,
 fmt_ns(projected_ns as u64)
 );

 // ── 4) §15 SecondaryDB catch-up lag p95 (proxy) ──
 //
 // XTDB single-node has no SecondaryDB; the closest analogue is the gap
 // between `submit-tx` (tx-log accept) and `await-tx` (index visible).
 // We measure that gap as a proxy. *Caveat*: this is **not** the §15
 // RocksDB SecondaryDB manifest-tail path; it bounds the same kind of
 // lag (write→visible) within XTDB but at a different layer.
 let mut lag = Histogram::<u64>::new(3).unwrap();
 let lag_iters = samples.min(500);
 let mut next_synth_id: u64 = 8_000_000;
 for _ in 0..lag_iters {
 let id = next_synth_id;
 next_synth_id += 1;
 let docs = vec![entity_doc(id, 1, "lag-probe")];
 let t_submit = Instant::now();
 let tx = client.submit_tx_puts(&docs).await?;
 let submit_done = t_submit.elapsed();
 let t_await = Instant::now();
 client.await_tx(tx).await?;
 let await_done = t_await.elapsed();
 // Lag is the await time alone (submit returned at submit_done).
 let _ = submit_done;
 lag.record(await_done.as_nanos() as u64).ok();
 }
 print_hist(
 "xtdb_index_lag_proxy (submit→visible)",
 &lag,
 "<100ms (proxy for §15)",
 );

 // ── Summary ──
 println!("\n--- §18 line 1924-1948 remaining 5 SLA (proxy / observed) ---");
 println!(
 "T1+T2 p95  {:>10} target <100ms",
 fmt_ns(t1t2.value_at_quantile(0.95))
 );
 if !epistemic.is_empty() {
 println!(
 "Epistemic p95 {:>10} target <50ms",
 fmt_ns(epistemic.value_at_quantile(0.95))
 );
 }
 println!(
 "Closure (observed) {:>10} target <1.25min @50K projection",
 fmt_ns(closure_p50)
 );
 println!(
 "Index lag p95 {:>10} target <100ms (PROXY: not §15 SecondaryDB)",
 fmt_ns(lag.value_at_quantile(0.95))
 );
 println!("Storage growth @200K — see `storage-growth` subcommand + `du`");
 let _ = workload; // keep workload alive for future scenarios
 Ok(())
}

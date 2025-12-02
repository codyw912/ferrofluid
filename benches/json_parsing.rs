//! Benchmarks comparing JSON parsing libraries for Hyperliquid API payloads
//!
//! Run with: cargo bench --bench json_parsing
//!
//! For best results, ensure you're compiling with:
//!   RUSTFLAGS="-C target-cpu=native" cargo bench --bench json_parsing

use criterion::{
    BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Types for benchmarking - mirrors the actual API types
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct L2Book {
    pub data: L2BookData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct L2BookData {
    pub coin: String,
    pub time: u64,
    pub levels: Vec<Vec<BookLevel>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BookLevel {
    pub px: String,
    pub sz: String,
    pub n: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Trades {
    pub data: Vec<Trade>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Trade {
    pub coin: String,
    pub side: String,
    pub px: String,
    pub sz: String,
    pub time: u64,
    pub hash: String,
    pub tid: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AllMids {
    pub data: AllMidsData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AllMidsData {
    pub mids: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderUpdate {
    pub order: BasicOrder,
    pub status: String,
    pub status_timestamp: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BasicOrder {
    pub coin: String,
    pub side: String,
    pub limit_px: String,
    pub sz: String,
    pub oid: u64,
    pub timestamp: u64,
    pub orig_sz: String,
    pub cloid: Option<String>,
}

// ============================================================================
// Sample payloads - realistic data matching Hyperliquid API responses
// ============================================================================

/// L2 order book with 20 levels on each side (typical depth)
fn sample_l2_book() -> String {
    let mut levels_bid = Vec::new();
    let mut levels_ask = Vec::new();

    for i in 0..20 {
        let bid_price = 50000.0 - (i as f64 * 10.0);
        let ask_price = 50010.0 + (i as f64 * 10.0);
        levels_bid.push(format!(
            r#"{{"px":"{}","sz":"{}","n":{}}}"#,
            bid_price,
            1.5 + (i as f64 * 0.1),
            i + 1
        ));
        levels_ask.push(format!(
            r#"{{"px":"{}","sz":"{}","n":{}}}"#,
            ask_price,
            2.0 + (i as f64 * 0.1),
            i + 1
        ));
    }

    format!(
        r#"{{"channel":"l2Book","data":{{"coin":"BTC","time":1699000000000,"levels":[[{}],[{}]]}}}}"#,
        levels_bid.join(","),
        levels_ask.join(",")
    )
}

/// Batch of recent trades (typical batch size)
fn sample_trades() -> String {
    let mut trades = Vec::new();

    for i in 0..50 {
        let side = if i % 2 == 0 { "A" } else { "B" };
        let price = 50000.0 + (i as f64 * 0.5);
        trades.push(format!(
            r#"{{"coin":"BTC","side":"{}","px":"{}","sz":"{}","time":{},"hash":"0x{}","tid":{}}}"#,
            side, price, 0.1 + (i as f64 * 0.01), 1699000000000u64 + i, format!("{:064x}", i), i
        ));
    }

    format!(r#"{{"channel":"trades","data":[{}]}}"#, trades.join(","))
}

/// All mid prices for ~200 assets
fn sample_all_mids() -> String {
    let mut mids = Vec::new();

    let coins = [
        "BTC", "ETH", "SOL", "AVAX", "MATIC", "ARB", "OP", "APT", "SUI", "SEI", "DOGE",
        "SHIB", "PEPE", "WIF", "BONK", "FLOKI", "MEME", "TURBO", "COQ", "MYRO", "LINK",
        "UNI", "AAVE", "MKR", "SNX", "CRV", "LDO", "RPL", "FXS", "CVX", "ATOM", "DOT",
        "ADA", "XRP", "TRX", "TON", "NEAR", "FTM", "ONE", "KAVA",
    ];

    for (i, coin) in coins.iter().enumerate() {
        let base_price = match *coin {
            "BTC" => 50000.0,
            "ETH" => 3000.0,
            _ => 10.0 + (i as f64),
        };
        mids.push(format!(r#""{}":"{}""#, coin, base_price));
    }

    // Add more synthetic coins to reach ~200
    for i in 0..160 {
        mids.push(format!(r#""COIN{}":"{}""#, i, 1.0 + (i as f64 * 0.01)));
    }

    format!(
        r#"{{"channel":"allMids","data":{{"mids":{{{}}}}}}}"#,
        mids.join(",")
    )
}

/// Order updates batch
fn sample_order_updates() -> String {
    let mut updates = Vec::new();

    for i in 0..10 {
        let side = if i % 2 == 0 { "A" } else { "B" };
        updates.push(format!(
            r#"{{"order":{{"coin":"BTC","side":"{}","limitPx":"{}","sz":"{}","oid":{},"timestamp":{},"origSz":"{}","cloid":null}},"status":"open","statusTimestamp":{}}}"#,
            side, 50000.0 + (i as f64 * 100.0), 0.1, 1000 + i, 1699000000000u64 + i, 0.1, 1699000000000u64 + i
        ));
    }

    format!(
        r#"{{"channel":"orderUpdates","data":[{}]}}"#,
        updates.join(",")
    )
}

// ============================================================================
// Wrapper type for the full message (tagged enum)
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "channel", rename_all = "camelCase")]
pub enum Message {
    L2Book(L2Book),
    Trades(Trades),
    AllMids(AllMids),
    OrderUpdates { data: Vec<OrderUpdate> },
}

// ============================================================================
// Benchmarks
// ============================================================================

fn bench_l2_book_deserialize(c: &mut Criterion) {
    let payload = sample_l2_book();
    let payload_bytes = payload.as_bytes();

    let mut group = c.benchmark_group("l2_book_deserialize");
    group.throughput(Throughput::Bytes(payload_bytes.len() as u64));

    group.bench_function("serde_json", |b| {
        b.iter(|| {
            let result: Message = serde_json::from_str(black_box(&payload)).unwrap();
            black_box(result)
        })
    });

    group.bench_function("simd_json", |b| {
        b.iter(|| {
            let mut data = payload_bytes.to_vec();
            let result: Message = simd_json::from_slice(black_box(&mut data)).unwrap();
            black_box(result)
        })
    });

    group.bench_function("sonic_rs", |b| {
        b.iter(|| {
            let result: Message = sonic_rs::from_str(black_box(&payload)).unwrap();
            black_box(result)
        })
    });

    group.finish();
}

fn bench_trades_deserialize(c: &mut Criterion) {
    let payload = sample_trades();
    let payload_bytes = payload.as_bytes();

    let mut group = c.benchmark_group("trades_deserialize");
    group.throughput(Throughput::Bytes(payload_bytes.len() as u64));

    group.bench_function("serde_json", |b| {
        b.iter(|| {
            let result: Message = serde_json::from_str(black_box(&payload)).unwrap();
            black_box(result)
        })
    });

    group.bench_function("simd_json", |b| {
        b.iter(|| {
            let mut data = payload_bytes.to_vec();
            let result: Message = simd_json::from_slice(black_box(&mut data)).unwrap();
            black_box(result)
        })
    });

    group.bench_function("sonic_rs", |b| {
        b.iter(|| {
            let result: Message = sonic_rs::from_str(black_box(&payload)).unwrap();
            black_box(result)
        })
    });

    group.finish();
}

fn bench_all_mids_deserialize(c: &mut Criterion) {
    let payload = sample_all_mids();
    let payload_bytes = payload.as_bytes();

    let mut group = c.benchmark_group("all_mids_deserialize");
    group.throughput(Throughput::Bytes(payload_bytes.len() as u64));

    group.bench_function("serde_json", |b| {
        b.iter(|| {
            let result: Message = serde_json::from_str(black_box(&payload)).unwrap();
            black_box(result)
        })
    });

    group.bench_function("simd_json", |b| {
        b.iter(|| {
            let mut data = payload_bytes.to_vec();
            let result: Message = simd_json::from_slice(black_box(&mut data)).unwrap();
            black_box(result)
        })
    });

    group.bench_function("sonic_rs", |b| {
        b.iter(|| {
            let result: Message = sonic_rs::from_str(black_box(&payload)).unwrap();
            black_box(result)
        })
    });

    group.finish();
}

fn bench_order_updates_deserialize(c: &mut Criterion) {
    let payload = sample_order_updates();
    let payload_bytes = payload.as_bytes();

    let mut group = c.benchmark_group("order_updates_deserialize");
    group.throughput(Throughput::Bytes(payload_bytes.len() as u64));

    group.bench_function("serde_json", |b| {
        b.iter(|| {
            let result: Message = serde_json::from_str(black_box(&payload)).unwrap();
            black_box(result)
        })
    });

    group.bench_function("simd_json", |b| {
        b.iter(|| {
            let mut data = payload_bytes.to_vec();
            let result: Message = simd_json::from_slice(black_box(&mut data)).unwrap();
            black_box(result)
        })
    });

    group.bench_function("sonic_rs", |b| {
        b.iter(|| {
            let result: Message = sonic_rs::from_str(black_box(&payload)).unwrap();
            black_box(result)
        })
    });

    group.finish();
}

/// Benchmark different payload sizes for L2 book
fn bench_l2_book_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("l2_book_scaling");

    for depth in [5, 10, 20, 50, 100] {
        // Generate L2 book with specified depth
        let mut levels_bid = Vec::new();
        let mut levels_ask = Vec::new();

        for i in 0..depth {
            let bid_price = 50000.0 - (i as f64 * 10.0);
            let ask_price = 50010.0 + (i as f64 * 10.0);
            levels_bid.push(format!(
                r#"{{"px":"{}","sz":"{}","n":{}}}"#,
                bid_price,
                1.5 + (i as f64 * 0.1),
                i + 1
            ));
            levels_ask.push(format!(
                r#"{{"px":"{}","sz":"{}","n":{}}}"#,
                ask_price,
                2.0 + (i as f64 * 0.1),
                i + 1
            ));
        }

        let payload = format!(
            r#"{{"channel":"l2Book","data":{{"coin":"BTC","time":1699000000000,"levels":[[{}],[{}]]}}}}"#,
            levels_bid.join(","),
            levels_ask.join(",")
        );
        let payload_bytes = payload.as_bytes();

        group.throughput(Throughput::Bytes(payload_bytes.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("sonic_rs", format!("depth_{}", depth)),
            &payload,
            |b, payload| {
                b.iter(|| {
                    let result: Message = sonic_rs::from_str(black_box(payload)).unwrap();
                    black_box(result)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("simd_json", format!("depth_{}", depth)),
            &payload,
            |b, payload| {
                b.iter(|| {
                    let mut data = payload.as_bytes().to_vec();
                    let result: Message =
                        simd_json::from_slice(black_box(&mut data)).unwrap();
                    black_box(result)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("serde_json", format!("depth_{}", depth)),
            &payload,
            |b, payload| {
                b.iter(|| {
                    let result: Message =
                        serde_json::from_str(black_box(payload)).unwrap();
                    black_box(result)
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// Types for serialization benchmarks - mirrors outbound request types
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct OrderRequest {
    #[serde(rename = "a")]
    pub asset: u32,
    #[serde(rename = "b")]
    pub is_buy: bool,
    #[serde(rename = "p")]
    pub limit_px: String,
    #[serde(rename = "s")]
    pub sz: String,
    #[serde(rename = "r")]
    pub reduce_only: bool,
    #[serde(rename = "t")]
    pub order_type: OrderTypeSer,
    #[serde(rename = "c", skip_serializing_if = "Option::is_none")]
    pub cloid: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OrderTypeSer {
    Limit(LimitSer),
    Trigger(TriggerSer),
}

#[derive(Debug, Clone, Serialize)]
pub struct LimitSer {
    pub tif: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerSer {
    #[serde(rename = "triggerPx")]
    pub trigger_px: String,
    #[serde(rename = "isMarket")]
    pub is_market: bool,
    pub tpsl: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkOrder {
    pub orders: Vec<OrderRequest>,
    pub grouping: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub builder: Option<BuilderInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BuilderInfo {
    #[serde(rename = "b")]
    pub builder: String,
    #[serde(rename = "f")]
    pub fee: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CancelRequest {
    #[serde(rename = "a")]
    pub asset: u32,
    #[serde(rename = "o")]
    pub oid: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkCancel {
    pub cancels: Vec<CancelRequest>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeRequest {
    pub action: Action,
    pub nonce: u64,
    pub signature: Signature,
    pub vault_address: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum Action {
    #[serde(rename = "order")]
    Order(BulkOrder),
    #[serde(rename = "cancel")]
    Cancel(BulkCancel),
}

#[derive(Debug, Clone, Serialize)]
pub struct Signature {
    pub r: String,
    pub s: String,
    pub v: u8,
}

// ============================================================================
// Sample data generators for serialization benchmarks
// ============================================================================

fn sample_order_request() -> OrderRequest {
    OrderRequest {
        asset: 0,
        is_buy: true,
        limit_px: "50000.0".to_string(),
        sz: "0.1".to_string(),
        reduce_only: false,
        order_type: OrderTypeSer::Limit(LimitSer {
            tif: "Gtc".to_string(),
        }),
        cloid: Some("00000000000000000000000000000001".to_string()),
    }
}

fn sample_bulk_order(count: usize) -> BulkOrder {
    let orders: Vec<OrderRequest> = (0..count)
        .map(|i| OrderRequest {
            asset: (i % 10) as u32,
            is_buy: i % 2 == 0,
            limit_px: format!("{}", 50000.0 + (i as f64 * 10.0)),
            sz: format!("{}", 0.1 + (i as f64 * 0.01)),
            reduce_only: false,
            order_type: OrderTypeSer::Limit(LimitSer {
                tif: "Gtc".to_string(),
            }),
            cloid: Some(format!("{:032x}", i)),
        })
        .collect();

    BulkOrder {
        orders,
        grouping: "na".to_string(),
        builder: Some(BuilderInfo {
            builder: "0x1234567890123456789012345678901234567890".to_string(),
            fee: 10,
        }),
    }
}

fn sample_bulk_cancel(count: usize) -> BulkCancel {
    let cancels: Vec<CancelRequest> = (0..count)
        .map(|i| CancelRequest {
            asset: (i % 10) as u32,
            oid: 1000000 + i as u64,
        })
        .collect();

    BulkCancel { cancels }
}

fn sample_exchange_request() -> ExchangeRequest {
    ExchangeRequest {
        action: Action::Order(sample_bulk_order(5)),
        nonce: 1699000000000,
        signature: Signature {
            r: "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
                .to_string(),
            s: "0xfedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321"
                .to_string(),
            v: 27,
        },
        vault_address: None,
    }
}

// ============================================================================
// Serialization Benchmarks
// ============================================================================

fn bench_single_order_serialize(c: &mut Criterion) {
    let order = sample_order_request();

    let mut group = c.benchmark_group("single_order_serialize");

    // Pre-serialize to measure throughput
    let serialized = serde_json::to_string(&order).unwrap();
    group.throughput(Throughput::Bytes(serialized.len() as u64));

    group.bench_function("serde_json", |b| {
        b.iter(|| {
            let result = serde_json::to_string(black_box(&order)).unwrap();
            black_box(result)
        })
    });

    group.bench_function("simd_json", |b| {
        b.iter(|| {
            let result = simd_json::to_string(black_box(&order)).unwrap();
            black_box(result)
        })
    });

    group.bench_function("sonic_rs", |b| {
        b.iter(|| {
            let result = sonic_rs::to_string(black_box(&order)).unwrap();
            black_box(result)
        })
    });

    group.finish();
}

fn bench_bulk_order_serialize(c: &mut Criterion) {
    let bulk_order = sample_bulk_order(10);

    let mut group = c.benchmark_group("bulk_order_serialize_10");

    let serialized = serde_json::to_string(&bulk_order).unwrap();
    group.throughput(Throughput::Bytes(serialized.len() as u64));

    group.bench_function("serde_json", |b| {
        b.iter(|| {
            let result = serde_json::to_string(black_box(&bulk_order)).unwrap();
            black_box(result)
        })
    });

    group.bench_function("simd_json", |b| {
        b.iter(|| {
            let result = simd_json::to_string(black_box(&bulk_order)).unwrap();
            black_box(result)
        })
    });

    group.bench_function("sonic_rs", |b| {
        b.iter(|| {
            let result = sonic_rs::to_string(black_box(&bulk_order)).unwrap();
            black_box(result)
        })
    });

    group.finish();
}

fn bench_bulk_cancel_serialize(c: &mut Criterion) {
    let bulk_cancel = sample_bulk_cancel(20);

    let mut group = c.benchmark_group("bulk_cancel_serialize_20");

    let serialized = serde_json::to_string(&bulk_cancel).unwrap();
    group.throughput(Throughput::Bytes(serialized.len() as u64));

    group.bench_function("serde_json", |b| {
        b.iter(|| {
            let result = serde_json::to_string(black_box(&bulk_cancel)).unwrap();
            black_box(result)
        })
    });

    group.bench_function("simd_json", |b| {
        b.iter(|| {
            let result = simd_json::to_string(black_box(&bulk_cancel)).unwrap();
            black_box(result)
        })
    });

    group.bench_function("sonic_rs", |b| {
        b.iter(|| {
            let result = sonic_rs::to_string(black_box(&bulk_cancel)).unwrap();
            black_box(result)
        })
    });

    group.finish();
}

fn bench_exchange_request_serialize(c: &mut Criterion) {
    let request = sample_exchange_request();

    let mut group = c.benchmark_group("exchange_request_serialize");

    let serialized = serde_json::to_string(&request).unwrap();
    group.throughput(Throughput::Bytes(serialized.len() as u64));

    group.bench_function("serde_json", |b| {
        b.iter(|| {
            let result = serde_json::to_string(black_box(&request)).unwrap();
            black_box(result)
        })
    });

    group.bench_function("simd_json", |b| {
        b.iter(|| {
            let result = simd_json::to_string(black_box(&request)).unwrap();
            black_box(result)
        })
    });

    group.bench_function("sonic_rs", |b| {
        b.iter(|| {
            let result = sonic_rs::to_string(black_box(&request)).unwrap();
            black_box(result)
        })
    });

    group.finish();
}

/// Benchmark serialization scaling with order batch size
fn bench_bulk_order_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("bulk_order_scaling");

    for count in [1, 5, 10, 20, 50] {
        let bulk_order = sample_bulk_order(count);
        let serialized = serde_json::to_string(&bulk_order).unwrap();

        group.throughput(Throughput::Bytes(serialized.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("sonic_rs", format!("orders_{}", count)),
            &bulk_order,
            |b, bulk_order| {
                b.iter(|| {
                    let result = sonic_rs::to_string(black_box(bulk_order)).unwrap();
                    black_box(result)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("simd_json", format!("orders_{}", count)),
            &bulk_order,
            |b, bulk_order| {
                b.iter(|| {
                    let result = simd_json::to_string(black_box(bulk_order)).unwrap();
                    black_box(result)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("serde_json", format!("orders_{}", count)),
            &bulk_order,
            |b, bulk_order| {
                b.iter(|| {
                    let result = serde_json::to_string(black_box(bulk_order)).unwrap();
                    black_box(result)
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    // Deserialization benchmarks
    bench_l2_book_deserialize,
    bench_trades_deserialize,
    bench_all_mids_deserialize,
    bench_order_updates_deserialize,
    bench_l2_book_scaling,
    // Serialization benchmarks
    bench_single_order_serialize,
    bench_bulk_order_serialize,
    bench_bulk_cancel_serialize,
    bench_exchange_request_serialize,
    bench_bulk_order_scaling,
);
criterion_main!(benches);

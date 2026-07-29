//! hiquant: 个人量化交易系统命令行
//!
//! 安装：
//!   cargo install --path crates/hiquant-cli
//!
//! 常用：
//!   hiquant sync 000001 --days 180        # 同步样本数据到数据库
//!   hiquant codes                         # 列出已存标的
//!   hiquant bars 000001                   # 查询 K 线
//!   hiquant bt 000001 --fast 5 --slow 20  # SMA 双均线回测
//!   hiquant web                           # 启动 Web 界面（含前端 + 模拟行情）
//!   hiquant server                        # 启动纯后端服务（可选 --static-dir）
//!   hiquant mock                          # MockBroker 下单演示
//!   hiquant ping --url http://x.x.x.x:7788  # 测 miniqmt sidecar

use anyhow::Result;
use clap::{Parser, Subcommand};
use hiquant_core::{Date, Frequency};
use hiquant_engine::{BacktestConfig, BacktestEngine, SmaCrossStrategy, SmaParams};
use hiquant_storage::{MarketStore, StoreConfig};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(
    name = "hiquant",
    version,
    about = "个人量化交易系统 (Hiquant)",
    long_about = None
)]
struct Cli {
    /// 数据库路径
    #[arg(long, global = true, default_value = "hiquant.db")]
    pub db: String,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    // ── 数据管理（完整子命令，保持层级） ──────────────────────────
    /// 行情数据管理 (sync / codes / bars)
    Data {
        #[command(subcommand)]
        action: DataAction,
    },
    // ── 数据：短别名，直接当顶层命令 ────────────────────────────
    /// 同步样本数据到数据库 (data sync 的别名)
    Sync {
        code: String,
        #[arg(long, default_value_t = 120)]
        days: usize,
        #[arg(long, default_value_t = 42)]
        seed: u64,
    },
    /// 列出已存储标的 (data codes 的别名)
    Codes,
    /// 查询 K 线 (data bars 的别名)
    Bars {
        code: String,
        #[arg(long, default_value = "2000-01-01")]
        start: String,
        #[arg(long, default_value = "2099-12-31")]
        end: String,
        #[arg(long, default_value = "day")]
        freq: String,
    },
    // ── 回测 ─────────────────────────────────────────────────
    /// 运行 SMA 双均线回测
    Backtest {
        code: String,
        #[arg(long, default_value_t = 5)]
        fast: usize,
        #[arg(long, default_value_t = 20)]
        slow: usize,
        #[arg(long, default_value_t = 100.0)]
        lots: f64,
        #[arg(long, default_value_t = 1_000_000.0)]
        cash: f64,
        #[arg(long, default_value_t = 120)]
        days: usize,
        #[arg(long, default_value_t = 42)]
        seed: u64,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// SMA 回测短别名 (backtest 的别名)
    Bt {
        code: String,
        #[arg(long, default_value_t = 5)]
        fast: usize,
        #[arg(long, default_value_t = 20)]
        slow: usize,
        #[arg(long, default_value_t = 100.0)]
        lots: f64,
        #[arg(long, default_value_t = 1_000_000.0)]
        cash: f64,
        #[arg(long, default_value_t = 120)]
        days: usize,
        #[arg(long, default_value_t = 42)]
        seed: u64,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    // ── 服务端 ───────────────────────────────────────────────
    /// 启动 HTTP + WebSocket 后端服务
    Server {
        #[arg(long, default_value = "0.0.0.0:7788")]
        addr: String,
        /// 静态前端目录（不指定则只提供 API / WS，不托管页面）
        #[arg(long)]
        static_dir: Option<PathBuf>,
        #[arg(long, default_value_t = 1_000_000.0)]
        cash: f64,
        /// 启动模拟行情推送
        #[arg(long)]
        mock_feed: bool,
    },
    /// 一键启动 Web 界面 (后端 + 前端页面 + 模拟行情)
    ///
    /// 等价于 server --static-dir <自动探测> --mock-feed
    Web {
        #[arg(long, default_value = "0.0.0.0:7788")]
        addr: String,
        /// 显式指定静态前端目录（默认自动探测：<repo>/crates/hiquant-server/static）
        #[arg(long)]
        static_dir: Option<PathBuf>,
        #[arg(long, default_value_t = 1_000_000.0)]
        cash: f64,
        /// 关闭模拟行情推送
        #[arg(long)]
        no_mock_feed: bool,
    },
    // ── Broker ────────────────────────────────────────────────
    /// Broker 联调 (test / ping)
    Broker {
        #[command(subcommand)]
        action: BrokerAction,
    },
    /// MockBroker 下单演示 (broker test 的别名)
    Mock,
    /// 测试 miniqmt sidecar 连接 (broker ping 的别名)
    Ping {
        #[arg(long, default_value = "http://127.0.0.1:7788")]
        url: String,
    },
}

#[derive(Subcommand, Debug)]
enum DataAction {
    Sync {
        code: String,
        #[arg(long, default_value_t = 120)]
        days: usize,
        #[arg(long, default_value_t = 42)]
        seed: u64,
    },
    Codes,
    Bars {
        code: String,
        #[arg(long, default_value = "2000-01-01")]
        start: String,
        #[arg(long, default_value = "2099-12-31")]
        end: String,
        #[arg(long, default_value = "day")]
        freq: String,
    },
}

#[derive(Subcommand, Debug)]
enum BrokerAction {
    Test,
    Ping {
        #[arg(long, default_value = "http://127.0.0.1:7788")]
        url: String,
    },
}

fn open_store(db: &str) -> Result<Arc<MarketStore>> {
    Ok(Arc::new(MarketStore::open(StoreConfig::new(db))?))
}

/// 自动探测前端静态目录
///
/// 查找顺序（找到即返回）：
///   1. 相对当前目录的 ./crates/hiquant-server/static
///   2. 相对 CARGO_MANIFEST_DIR/../../..（从 hiquant-cli 源码出发，workspace 根）
///   3. 可执行文件所在目录的 ../share/hiquant/static
fn auto_static_dir() -> Option<PathBuf> {
    // 1. 当前目录下（用户在仓库根运行）
    let p = Path::new("crates/hiquant-server/static");
    if p.join("index.html").exists() {
        return Some(p.to_path_buf());
    }
    // 2. 从 hiquant-cli 的 Cargo.toml 位置推断 workspace 根
    if let Ok(mdir) = std::env::var("CARGO_MANIFEST_DIR") {
        let cand = Path::new(&mdir)
            .join("../../crates/hiquant-server/static");
        if cand.join("index.html").exists() {
            return Some(cand);
        }
    }
    // 3. 安装后：bin 同级的 share/hiquant/static
    if let Ok(exe) = std::env::current_exe() {
        if let Some(bin_parent) = exe.parent() {
            let cand = bin_parent.join("../share/hiquant/static");
            if cand.join("index.html").exists() {
                return Some(cand);
            }
        }
    }
    None
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hiquant=info".into()),
        )
        .init();

    let cli = Cli::parse();

    match &cli.command {
        // ── 数据（完整形式） ──────────────────────────────────
        Commands::Data { action } => match action {
            DataAction::Sync { code, days, seed } => {
                cmd_sync(&cli.db, code, *days, *seed)?;
            }
            DataAction::Codes => cmd_codes(&cli.db)?,
            DataAction::Bars { code, start, end, freq } => {
                cmd_bars(&cli.db, code, start, end, freq)?;
            }
        },
        // ── 数据（短别名） ────────────────────────────────────
        Commands::Sync { code, days, seed } => cmd_sync(&cli.db, code, *days, *seed)?,
        Commands::Codes => cmd_codes(&cli.db)?,
        Commands::Bars { code, start, end, freq } => cmd_bars(&cli.db, code, start, end, freq)?,

        // ── 回测 ─────────────────────────────────────────────
        Commands::Backtest { code, fast, slow, lots, cash, days, seed, out } => {
            cmd_backtest(code, *fast, *slow, *lots, *cash, *days, *seed, out, &cli.db)?;
        }
        Commands::Bt { code, fast, slow, lots, cash, days, seed, out } => {
            cmd_backtest(code, *fast, *slow, *lots, *cash, *days, *seed, out, &cli.db)?;
        }

        // ── 服务端 ───────────────────────────────────────────
        Commands::Server { addr, static_dir, cash, mock_feed } => {
            cmd_server(addr, static_dir.clone(), *cash, *mock_feed, &cli.db).await?;
        }
        Commands::Web { addr, static_dir, cash, no_mock_feed } => {
            let dir = static_dir.clone().or_else(auto_static_dir);
            if dir.is_none() {
                tracing::warn!(
                    "未找到前端静态目录，使用 hiquant server --static-dir <path> 手动指定；当前只提供 API"
                );
            } else {
                tracing::info!(
                    "托管前端目录: {}",
                    dir.as_ref().unwrap().display()
                );
            }
            cmd_server(addr, dir, *cash, !*no_mock_feed, &cli.db).await?;
        }

        // ── Broker（完整形式） ───────────────────────────────
        Commands::Broker { action } => match action {
            BrokerAction::Test => cmd_mock_broker_test().await?,
            BrokerAction::Ping { url } => cmd_miniqmt_ping(url).await?,
        },
        // ── Broker（短别名） ─────────────────────────────────
        Commands::Mock => cmd_mock_broker_test().await?,
        Commands::Ping { url } => cmd_miniqmt_ping(url).await?,
    }
    Ok(())
}

// ───────────── 各子命令实现 ────────────────────────────────────

fn cmd_sync(db: &str, code: &str, days: usize, seed: u64) -> Result<()> {
    let store = open_store(db)?;
    let mut gen = hiquant_data::SampleDataGenerator::new(seed);
    let bars = gen.gen_daily(code, Date::from_ymd(2024, 1, 2), days, 10.0, 0.10, 0.30, false);
    let res = store.upsert_bars(code, Frequency::Day, &bars)?;
    println!(
        "synced {} bars for {}, last_dt={:?}",
        res.inserted, code, res.last_datetime
    );
    Ok(())
}

fn cmd_codes(db: &str) -> Result<()> {
    let store = open_store(db)?;
    let codes = store.list_codes()?;
    println!("codes ({}):", codes.len());
    for c in codes {
        println!("  {c}");
    }
    Ok(())
}

fn cmd_bars(db: &str, code: &str, start: &str, end: &str, freq: &str) -> Result<()> {
    let store = open_store(db)?;
    let f = hiquant_core::Frequency::from_str(freq).unwrap_or(Frequency::Day);
    let bars = store.query_bars(code, f, start, end)?;
    println!("bars for {} ({} rows):", code, bars.len());
    for b in bars.iter().take(5) {
        println!(
            "  {} O:{:.2} H:{:.2} L:{:.2} C:{:.2} V:{:.0}",
            b.datetime, b.open, b.high, b.low, b.close, b.volume
        );
    }
    if bars.len() > 5 {
        println!("  ... ({} more)", bars.len() - 5);
    }
    Ok(())
}

fn cmd_backtest(
    code: &str,
    fast: usize,
    slow: usize,
    lots: f64,
    cash: f64,
    days: usize,
    seed: u64,
    out: &Option<PathBuf>,
    db: &str,
) -> Result<()> {
    let store = open_store(db)?;
    let bars = match store.query_bars(code, Frequency::Day, "2000-01-01", "2099-12-31") {
        Ok(b) if !b.is_empty() => b,
        _ => {
            tracing::info!("no data in store, generating {days} sample bars for {code}");
            let mut gen = hiquant_data::SampleDataGenerator::new(seed);
            gen.gen_daily(code, Date::from_ymd(2024, 1, 2), days, 10.0, 0.10, 0.30, false)
        }
    };

    let params = SmaParams {
        code: code.to_string(),
        fast,
        slow,
        lots,
    };
    let strategy = Box::new(SmaCrossStrategy::new(params));
    let cfg = BacktestConfig {
        account_cookie: format!("bt_{code}"),
        init_cash: cash,
        environment: hiquant_core::AccountEnvironment::Backtest,
    };
    let mut engine = BacktestEngine::new(cfg, strategy);
    engine.load_bars(code.to_string(), bars);
    let result = engine.run();

    let p = &result.performance;
    println!("==== Backtest: {} (SMA {}/{}) ====", code, fast, slow);
    println!("init_cash      : {:.2}", p.init_cash);
    println!("final_value    : {:.2}", p.final_value);
    println!("total_return   : {:.2}%", p.total_return * 100.0);
    println!("annual_return  : {:.2}%", p.annual_return * 100.0);
    println!("max_drawdown   : {:.2}%", p.max_drawdown * 100.0);
    println!("sharpe         : {:.3}", p.sharpe);
    println!("trading_days   : {}", p.trading_days);
    println!("trade_count    : {}", p.trade_count);

    if let Some(path) = out {
        let json = serde_json::to_string_pretty(&result)?;
        std::fs::write(path, json)?;
        println!("\nresult written to {}", path.display());
    }
    Ok(())
}

async fn cmd_server(
    addr: &str,
    static_dir: Option<PathBuf>,
    cash: f64,
    mock_feed: bool,
    db: &str,
) -> Result<()> {
    let store = open_store(db)?;
    let market = Arc::new(hiquant_market::QAMarketSystem::new("hiquant"));
    market.register_account("default", cash);
    let state = Arc::new(hiquant_server::AppState::new(market, store, cash));

    if mock_feed {
        let st = state.clone();
        tokio::spawn(async move {
            let market = st.market.clone();
            let broadcaster = st.broadcaster.clone();
            let code = "000001".to_string();
            let mut price = 10.0_f64;
            let mut step = 0u64;
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                step += 1;
                let delta = ((step as f64).sin() * 0.05) + (step as f64 % 3.0 - 1.5) * 0.02;
                price = (price + delta).max(1.0);
                let now = chrono::Utc::now().to_rfc3339();
                market.update_price(&code, price);
                broadcaster.send(&hiquant_server::WsEvent::Price {
                    code: code.clone(),
                    price,
                    datetime: now,
                });
                for name in market.account_names() {
                    if let Some(acc) = market.get_account(&name) {
                        broadcaster.send(&hiquant_server::WsEvent::Account {
                            summary: acc.summary(),
                        });
                    }
                }
            }
        });
        tracing::info!("mock feed started");
    }

    let socket: std::net::SocketAddr = addr.parse()?;
    hiquant_server::serve(state, socket, static_dir).await?;
    Ok(())
}

async fn cmd_mock_broker_test() -> Result<()> {
    use hiquant_broker::{Broker, BrokerOrder, MockBroker, MockConfig};
    let mut quotes = std::collections::HashMap::new();
    quotes.insert("000001".to_string(), 10.0);
    let cfg = MockConfig {
        account_id: "mock".into(),
        init_cash: 100_000.0,
        quotes,
    };
    let broker = MockBroker::new(cfg);
    println!("broker connected: {}", broker.is_connected().await);
    let r = broker
        .place_order(BrokerOrder::buy("000001", 100.0, 10.0))
        .await?;
    println!("buy order: {:?}", r);
    broker.set_price("000001", 11.0);
    let acc = broker.query_account().await?;
    println!(
        "account: balance={:.2} available={:.2}",
        acc.balance, acc.available
    );
    let r = broker
        .place_order(BrokerOrder::sell("000001", 100.0, 11.0))
        .await?;
    println!("sell order: {:?}", r);
    let trades = broker.query_trades().await?;
    println!("trades: {}", trades.len());
    Ok(())
}

async fn cmd_miniqmt_ping(url: &str) -> Result<()> {
    use hiquant_broker::{Broker, MiniQmtBroker, MiniQmtConfig};
    let broker = MiniQmtBroker::new(MiniQmtConfig {
        base_url: url.to_string(),
        ..Default::default()
    });
    let connected = broker.is_connected().await;
    println!("miniqmt sidecar {} connected={}", url, connected);
    if !connected {
        anyhow::bail!("sidecar not reachable at {url}");
    }
    Ok(())
}

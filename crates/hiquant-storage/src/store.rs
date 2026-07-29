//! SQLite 行情/账户存储实现

use chrono::Utc;
use hiquant_core::{Amount, Date, Frequency, Price, Result, Volume};
use hiquant_data::{Bar, Tick};
use hiquant_protocol::qifi::Qifi;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreConfig {
    pub path: String,
    /// 写入时是否同步刷盘（PRAGMA synchronous = FULL）
    pub sync: bool,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            path: "hiquant.db".to_string(),
            sync: false,
        }
    }
}

impl StoreConfig {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            sync: false,
        }
    }
}

/// 增量写入结果
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpsertResult {
    pub inserted: usize,
    pub updated: usize,
    pub last_datetime: Option<String>,
}

/// 账户快照记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSnapshot {
    pub id: i64,
    pub account_cookie: String,
    pub snapshot_time: String,
    pub qifi_json: String,
}

/// 行情/账户存储
pub struct MarketStore {
    config: StoreConfig,
    conn: Mutex<Connection>,
}

impl MarketStore {
    pub fn open(config: StoreConfig) -> Result<Self> {
        let path = PathBuf::from(&config.path);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    hiquant_core::HiquantError::Storage(format!("create dir: {e}"))
                })?;
            }
        }
        let conn = Connection::open(&config.path).map_err(|e| {
            hiquant_core::HiquantError::Storage(format!("open sqlite: {e}"))
        })?;
        if !config.sync {
            conn.pragma_update(None, "synchronous", "NORMAL")
                .map_err(rusql_err)?;
            conn.pragma_update(None, "journal_mode", "WAL")
                .map_err(rusql_err)?;
        }
        let store = Self {
            config,
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    /// 内存数据库（用于测试与临时缓存）
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(rusql_err)?;
        let store = Self {
            config: StoreConfig {
                path: ":memory:".to_string(),
                sync: false,
            },
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn config(&self) -> &StoreConfig {
        &self.config
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS bars (
                code       TEXT NOT NULL,
                freq       TEXT NOT NULL,
                datetime   TEXT NOT NULL,
                trading_date TEXT,
                open       REAL NOT NULL,
                high       REAL NOT NULL,
                low        REAL NOT NULL,
                close      REAL NOT NULL,
                volume     REAL NOT NULL,
                amount     REAL DEFAULT 0,
                pre_close  REAL DEFAULT 0,
                limit_up   REAL DEFAULT 0,
                limit_down REAL DEFAULT 0,
                open_interest REAL DEFAULT 0,
                PRIMARY KEY (code, freq, datetime)
            );

            CREATE INDEX IF NOT EXISTS idx_bars_code_freq_dt
                ON bars(code, freq, datetime);

            CREATE TABLE IF NOT EXISTS ticks (
                code     TEXT NOT NULL,
                datetime TEXT NOT NULL,
                last_price REAL NOT NULL,
                volume   REAL NOT NULL,
                amount   REAL DEFAULT 0,
                bid_json TEXT,
                ask_json TEXT,
                PRIMARY KEY (code, datetime)
            );

            CREATE TABLE IF NOT EXISTS data_sync_state (
                code      TEXT NOT NULL,
                freq      TEXT NOT NULL,
                last_datetime TEXT,
                last_sync_time TEXT,
                row_count INTEGER DEFAULT 0,
                PRIMARY KEY (code, freq)
            );

            CREATE TABLE IF NOT EXISTS account_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_cookie TEXT NOT NULL,
                snapshot_time  TEXT NOT NULL,
                qifi_json      TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_snapshots_cookie_time
                ON account_snapshots(account_cookie, snapshot_time);

            CREATE TABLE IF NOT EXISTS instruments (
                code        TEXT PRIMARY KEY,
                exchange_id TEXT,
                name        TEXT,
                market_type TEXT,
                price_tick  REAL DEFAULT 0.01,
                lot_size    REAL DEFAULT 100,
                multiplier  REAL DEFAULT 1,
                margin_rate REAL DEFAULT 1.0,
                commission_rate REAL DEFAULT 0.0003,
                list_date   TEXT,
                expire_date TEXT
            );
            "#,
        )
        .map_err(rusql_err)?;
        debug!("storage schema initialized");
        Ok(())
    }

    /// 增量写入 K 线：按主键 UPSERT，并更新同步状态
    pub fn upsert_bars(&self, code: &str, freq: Frequency, bars: &[Bar]) -> Result<UpsertResult> {
        if bars.is_empty() {
            return Ok(UpsertResult::default());
        }
        let freq_str = freq.as_str();
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction().map_err(rusql_err)?;

        let mut inserted = 0usize;
        let mut updated = 0usize;
        let mut last_dt: Option<String> = None;

        for b in bars {
            let td = b.trading_date.map(|d| d.as_str()).unwrap_or_default();
            let changed = tx
                .execute(
                    r#"INSERT INTO bars
                       (code, freq, datetime, trading_date, open, high, low, close,
                        volume, amount, pre_close, limit_up, limit_down, open_interest)
                       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                       ON CONFLICT(code, freq, datetime) DO UPDATE SET
                        trading_date=excluded.trading_date,
                        open=excluded.open, high=excluded.high, low=excluded.low, close=excluded.close,
                        volume=excluded.volume, amount=excluded.amount, pre_close=excluded.pre_close,
                        limit_up=excluded.limit_up, limit_down=excluded.limit_down,
                        open_interest=excluded.open_interest"#,
                    params![
                        code, freq_str, b.datetime, td,
                        b.open, b.high, b.low, b.close,
                        b.volume, b.amount, b.pre_close, b.limit_up, b.limit_down, b.open_interest,
                    ],
                )
                .map_err(rusql_err)?;
            if changed == 0 {
                // unchanged
            } else {
                // SQLite 不直接区分 INSERT 与 UPDATE，统一记为 upserted
                inserted += 1;
            }
            last_dt = Some(b.datetime.clone());
        }

        // 更新同步状态
        let row_count: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM bars WHERE code=?1 AND freq=?2",
                params![code, freq_str],
                |r| r.get(0),
            )
            .map_err(rusql_err)?;
        tx.execute(
            r#"INSERT INTO data_sync_state (code, freq, last_datetime, last_sync_time, row_count)
               VALUES (?1, ?2, ?3, ?4, ?5)
               ON CONFLICT(code, freq) DO UPDATE SET
                last_datetime=excluded.last_datetime,
                last_sync_time=excluded.last_sync_time,
                row_count=excluded.row_count"#,
            params![
                code,
                freq_str,
                last_dt.clone().unwrap_or_default(),
                Utc::now().to_rfc3339(),
                row_count
            ],
        )
        .map_err(rusql_err)?;

        tx.commit().map_err(rusql_err)?;
        info!(
            "upserted {} bars for {} {} last_dt={:?}",
            bars.len(),
            code,
            freq_str,
            last_dt
        );
        updated = inserted;
        Ok(UpsertResult {
            inserted,
            updated,
            last_datetime: last_dt,
        })
    }

    pub fn upsert_ticks(&self, code: &str, ticks: &[Tick]) -> Result<UpsertResult> {
        if ticks.is_empty() {
            return Ok(UpsertResult::default());
        }
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction().map_err(rusql_err)?;
        let mut last_dt: Option<String> = None;
        for t in ticks {
            let bid_json = serde_json::to_string(&t.bid_prices).unwrap_or_default();
            let ask_json = serde_json::to_string(&t.ask_prices).unwrap_or_default();
            tx.execute(
                r#"INSERT INTO ticks (code, datetime, last_price, volume, amount, bid_json, ask_json)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                   ON CONFLICT(code, datetime) DO UPDATE SET
                    last_price=excluded.last_price, volume=excluded.volume,
                    amount=excluded.amount, bid_json=excluded.bid_json, ask_json=excluded.ask_json"#,
                params![code, t.datetime, t.last_price, t.volume, t.amount, bid_json, ask_json],
            )
            .map_err(rusql_err)?;
            last_dt = Some(t.datetime.clone());
        }
        tx.commit().map_err(rusql_err)?;
        Ok(UpsertResult {
            inserted: ticks.len(),
            updated: ticks.len(),
            last_datetime: last_dt,
        })
    }

    /// 查询 K 线
    pub fn query_bars(
        &self,
        code: &str,
        freq: Frequency,
        start: &str,
        end: &str,
    ) -> Result<Vec<Bar>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT datetime, trading_date, open, high, low, close, volume, amount,
                        pre_close, limit_up, limit_down, open_interest
                 FROM bars
                 WHERE code=?1 AND freq=?2 AND datetime>=?3 AND datetime<=?4
                 ORDER BY datetime ASC",
            )
            .map_err(rusql_err)?;
        let rows = stmt
            .query_map(params![code, freq.as_str(), start, end], |r| {
                let dt: String = r.get(0)?;
                let td: Option<String> = r.get(1)?;
                Ok(Bar {
                    order_book_id: code.to_string(),
                    exchange_id: String::new(),
                    frequency: freq,
                    datetime: dt.clone(),
                    trading_date: td.and_then(|s| Date::parse(&s)),
                    open: r.get(2)?,
                    high: r.get(3)?,
                    low: r.get(4)?,
                    close: r.get(5)?,
                    volume: r.get(6)?,
                    amount: r.get(7)?,
                    pre_close: r.get(8)?,
                    limit_up: r.get(9)?,
                    limit_down: r.get(10)?,
                    open_interest: r.get(11)?,
                    split_coefficient_to: 1.0,
                    dividend_cash_before_tax: 0.0,
                })
            })
            .map_err(rusql_err)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(rusql_err)?);
        }
        Ok(out)
    }

    pub fn query_ticks(&self, code: &str, start: &str, end: &str) -> Result<Vec<Tick>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT datetime, last_price, volume, amount, bid_json, ask_json
                 FROM ticks WHERE code=?1 AND datetime>=?2 AND datetime<=?3 ORDER BY datetime ASC",
            )
            .map_err(rusql_err)?;
        let rows = stmt.query_map(params![code, start, end], |r| {
            let dt: String = r.get(0)?;
            let last_price: Price = r.get(1)?;
            let volume: Volume = r.get(2)?;
            let amount: Amount = r.get(3)?;
            let bid_json: String = r.get(4).unwrap_or_default();
            let ask_json: String = r.get(5).unwrap_or_default();
            let bid_prices: Vec<Price> =
                serde_json::from_str(&bid_json).unwrap_or_default();
            let ask_prices: Vec<Price> =
                serde_json::from_str(&ask_json).unwrap_or_default();
            Ok(Tick {
                instrument_id: code.to_string(),
                exchange_id: String::new(),
                datetime: dt,
                last_price,
                pre_close: 0.0,
                open: last_price,
                high: last_price,
                low: last_price,
                volume,
                amount,
                bid_prices,
                bid_volumes: Vec::new(),
                ask_prices,
                ask_volumes: Vec::new(),
                open_interest: 0.0,
                limit_up: 0.0,
                limit_down: 0.0,
            })
        }).map_err(rusql_err)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(rusql_err)?);
        }
        Ok(out)
    }

    /// 获取某个 (code, freq) 的最后同步日期，用于增量更新起点
    pub fn last_sync_datetime(&self, code: &str, freq: Frequency) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let dt: Option<String> = conn
            .query_row(
                "SELECT last_datetime FROM data_sync_state WHERE code=?1 AND freq=?2",
                params![code, freq.as_str()],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        Ok(dt)
    }

    /// 列出所有已存储的标的代码
    pub fn list_codes(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT DISTINCT code FROM bars ORDER BY code")
            .map_err(rusql_err)?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(rusql_err)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(rusql_err)?);
        }
        Ok(out)
    }

    /// 保存账户 QIFI 快照
    pub fn save_account_snapshot(&self, qifi: &Qifi) -> Result<i64> {
        let json = serde_json::to_string(qifi)
            .map_err(|e| hiquant_core::HiquantError::Storage(format!("qifi serialize: {e}")))?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO account_snapshots (account_cookie, snapshot_time, qifi_json) VALUES (?1, ?2, ?3)",
            params![qifi.account_cookie, Utc::now().to_rfc3339(), json],
        )
        .map_err(rusql_err)?;
        Ok(conn.last_insert_rowid())
    }

    pub fn load_latest_account_snapshot(&self, account_cookie: &str) -> Result<Option<Qifi>> {
        let conn = self.conn.lock().unwrap();
        let json: Option<String> = conn
            .query_row(
                "SELECT qifi_json FROM account_snapshots WHERE account_cookie=?1 ORDER BY snapshot_time DESC LIMIT 1",
                params![account_cookie],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        match json {
            Some(j) => {
                let qifi: Qifi = serde_json::from_str(&j).map_err(|e| {
                    hiquant_core::HiquantError::Storage(format!("qifi deserialize: {e}"))
                })?;
                Ok(Some(qifi))
            }
            None => Ok(None),
        }
    }

    pub fn list_account_snapshots(&self, account_cookie: &str) -> Result<Vec<AccountSnapshot>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, account_cookie, snapshot_time, qifi_json FROM account_snapshots
                 WHERE account_cookie=?1 ORDER BY snapshot_time DESC",
            )
            .map_err(rusql_err)?;
        let rows = stmt.query_map(params![account_cookie], |r| {
            Ok(AccountSnapshot {
                id: r.get(0)?,
                account_cookie: r.get(1)?,
                snapshot_time: r.get(2)?,
                qifi_json: r.get(3)?,
            })
        }).map_err(rusql_err)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(rusql_err)?);
        }
        Ok(out)
    }

    /// 保存/更新合约信息
    pub fn upsert_instrument(&self, info: &hiquant_protocol::mifi::InstrumentInfo) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"INSERT INTO instruments
               (code, exchange_id, name, market_type, price_tick, lot_size, multiplier,
                margin_rate, commission_rate, list_date, expire_date)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
               ON CONFLICT(code) DO UPDATE SET
                exchange_id=excluded.exchange_id, name=excluded.name,
                market_type=excluded.market_type, price_tick=excluded.price_tick,
                lot_size=excluded.lot_size, multiplier=excluded.multiplier,
                margin_rate=excluded.margin_rate, commission_rate=excluded.commission_rate,
                list_date=excluded.list_date, expire_date=excluded.expire_date"#,
            params![
                info.instrument_id,
                info.exchange_id,
                info.instrument_name,
                info.market_type,
                info.price_tick,
                info.lot_size,
                info.multiplier,
                info.margin_rate,
                info.commission_rate,
                info.list_date,
                info.expire_date,
            ],
        )
        .map_err(rusql_err)?;
        Ok(())
    }

    pub fn get_instrument(&self, code: &str) -> Result<Option<hiquant_protocol::mifi::InstrumentInfo>> {
        let conn = self.conn.lock().unwrap();
        let res = conn.query_row(
            "SELECT code, exchange_id, name, market_type, price_tick, lot_size, multiplier,
                    margin_rate, commission_rate, list_date, expire_date
             FROM instruments WHERE code=?1",
            params![code],
            |r| {
                Ok(hiquant_protocol::mifi::InstrumentInfo {
                    instrument_id: r.get(0)?,
                    exchange_id: r.get(1)?,
                    instrument_name: r.get(2)?,
                    market_type: r.get(3)?,
                    price_tick: r.get(4)?,
                    lot_size: r.get(5)?,
                    multiplier: r.get(6)?,
                    margin_rate: r.get(7)?,
                    commission_rate: r.get(8)?,
                    list_date: r.get(9)?,
                    expire_date: r.get(10)?,
                    limit_up_rate: 0.0,
                    limit_down_rate: 0.0,
                })
            },
        );
        match res {
            Ok(info) => Ok(Some(info)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(rusql_err(e)),
        }
    }
}

fn rusql_err(e: rusqlite::Error) -> hiquant_core::HiquantError {
    hiquant_core::HiquantError::Storage(format!("sqlite: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_and_query_bars() {
        let store = MarketStore::open_in_memory().unwrap();
        let mut gen = hiquant_data::SampleDataGenerator::new(1);
        let bars = gen.gen_daily("000001", Date::from_ymd(2024, 1, 2), 5, 10.0, 0.05, 0.25, false);
        let res = store.upsert_bars("000001", Frequency::Day, &bars).unwrap();
        assert_eq!(res.inserted, 5);
        let last = store.last_sync_datetime("000001", Frequency::Day).unwrap();
        assert!(last.is_some());

        // 增量更新：再加 3 根
        let bars2 = gen.gen_daily("000001", Date::from_ymd(2024, 1, 2), 8, 10.0, 0.05, 0.25, false);
        // 取后 3 根
        let new_bars: Vec<Bar> = bars2.into_iter().rev().take(3).rev().collect();
        let res2 = store.upsert_bars("000001", Frequency::Day, &new_bars).unwrap();
        assert!(res2.inserted >= 3);

        let q = store
            .query_bars("000001", Frequency::Day, "2024-01-01", "2025-12-31")
            .unwrap();
        assert!(q.len() >= 8);
    }

    #[test]
    fn snapshot_roundtrip() {
        let store = MarketStore::open_in_memory().unwrap();
        let mut qifi = Qifi::new("acc1", 100000.0);
        qifi.add_position(hiquant_protocol::qifi::Position {
            instrument_id: "000001".to_string(),
            volume_long_today: 100.0,
            ..Default::default()
        });
        store.save_account_snapshot(&qifi).unwrap();
        let loaded = store.load_latest_account_snapshot("acc1").unwrap().unwrap();
        assert_eq!(loaded.account_cookie, "acc1");
        assert_eq!(loaded.positions.len(), 1);
    }
}

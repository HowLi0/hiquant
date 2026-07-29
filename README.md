# Hiquant

个人量化交易系统（Rust 实现），支持本地数据回测、自建 SQLite 行情数据库（增量更新），以及通过 miniqmt/大QMT 实盘交易，附带 React + ECharts 前端。

## 工程结构

Cargo workspace 多 crate：

| crate | 职责 |
|-------|------|
| `hiquant-core` | 核心类型：`Price`/`Volume`/`Amount`、`Direction`/`OrderStatus`/`Frequency`、`Date`、错误类型 |
| `hiquant-protocol` | QIFI/MIFI/TIFI 序列化协议 |
| `hiquant-data` | `Bar`/`Tick`、可插拔 `DataSource`（CSV/HTTP/SQLite）、交易日历、样本数据生成器 |
| `hiquant-storage` | SQLite 行情/账户存储，支持按 `(code, freq, datetime)` 主键增量 UPSERT |
| `hiquant-account` | 账户/订单/持仓/合约预设，含风控、冻结、盈亏计算 |
| `hiquant-market` | 限价订单簿撮合引擎（价格/时间优先）+ `QAMarketSystem` 市场系统 |
| `hiquant-engine` | `Strategy` trait、`BacktestEngine`、SMA 双均线策略、净值曲线与绩效（夏普/回撤/年化） |
| `hiquant-broker` | `Broker` trait、`MockBroker`、`MiniQmtBroker`（HTTP sidecar 桥接 miniqmt） |
| `hiquant-server` | axum REST + WebSocket 服务，托管前端静态资源 |
| `hiquant-cli` | 命令行入口 `hiquant` |
| `frontend/` | React + Vite + ECharts 前端，构建产物输出到 `hiquant-server/static` |
| `python_sidecar/` | miniqmt Python sidecar 参考实现（xtquant 调用） |

## 快速开始

### 1. 安装（推荐：`hiquant` 直接进入 PATH）

```bash
# 1) 构建前端（输出到 crates/hiquant-server/static）
cd frontend && npm install && npm run build && cd ..

# 2) 把 hiquant 命令安装到 ~/.cargo/bin/hiquant（需要 PATH 里有 ~/.cargo/bin）
cargo install --path crates/hiquant-cli --locked

# 确认安装
hiquant --help

# 如果之后改了代码，重装即可
cargo install --path crates/hiquant-cli --locked --force
```

> 不想全局安装？下面所有命令把 `hiquant xxx` 换成 `cargo run -p hiquant-cli -- xxx`
> 或 `./target/release/hiquant xxx`（自己 `cargo build --release -p hiquant-cli` 后）也行。

### 2. 命令行使用

数据、回测、服务端、MockBroker 都有**短别名**（推荐）和长形式两种写法。

#### 数据管理

```bash
# 生成 180 天样本 K 线并写入 SQLite（无外部数据源时演示用）
hiquant sync 000001 --days 180 --seed 42
# 等价于: hiquant data sync 000001 --days 180 --seed 42

# 列出已存标的
hiquant codes
# 等价于: hiquant data codes

# 查询 K 线
hiquant bars 000001 --start 2024-01-01 --end 2024-12-31 --freq day
# 等价于: hiquant data bars 000001 ...
```

#### 回测

```bash
# SMA 5/20 双均线回测
hiquant bt 000001 --fast 5 --slow 20 --lots 100 --cash 1000000 --out bt.json
# 等价于: hiquant backtest 000001 --fast 5 --slow 20 --lots 100 ...
```

输出：
```
==== Backtest: 000001 (SMA 5/20) ====
init_cash      : 1000000.00
final_value    : ...
total_return   : ...%
annual_return  : ...%
max_drawdown   : ...%
sharpe         : ...
trading_days   : ...
trade_count    : ...
```

#### 启动 Web 界面（**推荐**：一行搞定）

```bash
# 一键启动：后端 + 前端页面（自动探测 static 目录）+ 模拟行情推送
hiquant web

# 浏览器打开 http://127.0.0.1:7788
```

高级参数：

```bash
# 指定地址 / 端口
hiquant web --addr 0.0.0.0:7788

# 不想开模拟行情
hiquant web --no-mock-feed

# 显式指定前端目录（不在仓库根目录运行时用）
hiquant web --static-dir /path/to/hiquant-server/static
```

#### 只启动纯后端服务（不托管前端）

```bash
hiquant server
# 等价于旧版 server（不带 --static-dir 就只提供 REST/WS API）
hiquant server --mock-feed --static-dir crates/hiquant-server/static
```

#### MockBroker 演示 / miniqmt 联通性

```bash
# MockBroker 演示买-持有-卖闭环
hiquant mock
# 等价于: hiquant broker test

# 测试 miniqmt Python sidecar 是否可连通
hiquant ping --url http://192.168.1.100:7788
# 等价于: hiquant broker ping --url ...
```

### 3. 对接 miniqmt 实盘

```bash
# Windows 端：启动 Python sidecar（需安装 xtquant）
cd python_sidecar
pip install flask xtquant
python miniqmt_sidecar.py --port 7788 --account YOUR_ACCOUNT_ID --path "C:\QMT\userdata_mini"

# Rust 端：测试 sidecar 可通
hiquant ping --url http://WINDOWS_IP:7788

# 通过 Rust 下单
# 1) 启动带 broker 的后端（生产环境请走内网 / 鉴权）
hiquant server --addr 127.0.0.1:7789
# 2) 前端调用 /api/broker/place_order 下单，或 CLI 写脚本用 MiniQmtBroker
```

## 前端功能

- **账户面板**：账户切换、新建账户、资金/持仓/盈亏实时展示
- **下单**：模拟账户买入/卖出
- **回测面板**：配置 SMA 参数 → 运行回测 → 净值曲线 + 指标曲线 + 绩效指标卡片
- **实时行情**：WebSocket 推送价格，ECharts 实时刷新
- **实盘经纪商**：展示 miniqmt 侧资金/持仓

## API 一览

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/health` | 健康检查 |
| GET | `/api/codes` | 已存储标的 |
| GET | `/api/bars?code=&freq=&start=&end=` | 查询 K 线 |
| GET/POST | `/api/accounts` | 列出/新建账户 |
| GET | `/api/accounts/:name/summary` | 账户资金摘要 |
| GET | `/api/accounts/:name/positions` | 账户持仓 |
| POST | `/api/orders` | 下单（模拟账户） |
| POST | `/api/backtest` | 运行回测 |
| POST | `/api/sync` | 同步样本数据 |
| POST | `/api/mock-feed` | 启动模拟行情推送 |
| GET | `/api/broker/account` | 实盘资金 |
| GET | `/api/broker/positions` | 实盘持仓 |
| POST | `/api/broker/place_order` | 实盘下单 |
| WS | `/ws` | 实时推送行情/账户/成交事件 |

## 测试

```bash
cargo test --workspace
```

## 命令速查

| 目标 | 推荐写法 | 完整写法 |
|------|----------|----------|
| 同步样本数据 | `hiquant sync CODE --days N` | `hiquant data sync CODE --days N` |
| 列标的 | `hiquant codes` | `hiquant data codes` |
| 查 K 线 | `hiquant bars CODE ...` | `hiquant data bars CODE ...` |
| SMA 回测 | `hiquant bt CODE --fast 5 --slow 20` | `hiquant backtest CODE --fast 5 --slow 20` |
| 开 Web 界面 | `hiquant web` | `hiquant server --static-dir ... --mock-feed` |
| 开纯后端 | `hiquant server` | `hiquant server` |
| Mock 下单 | `hiquant mock` | `hiquant broker test` |
| 测 sidecar | `hiquant ping --url ...` | `hiquant broker ping --url ...` |

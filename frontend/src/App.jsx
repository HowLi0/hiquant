import { useEffect, useState, useMemo, useCallback } from 'react';
import { api, connectWs } from './api.js';
import EChart from './components/EChart.jsx';

const fmt = (n, d = 2) =>
  n == null ? '-' : Number(n).toLocaleString('en-US', { minimumFractionDigits: d, maximumFractionDigits: d });
const pct = (n) => (n == null ? '-' : `${(n * 100).toFixed(2)}%`);

export default function App() {
  const [tab, setTab] = useState('backtest');
  const [live, setLive] = useState(false);
  const [accounts, setAccounts] = useState([]);
  const [activeAccount, setActiveAccount] = useState('');
  const [summary, setSummary] = useState(null);
  const [positions, setPositions] = useState([]);
  const [trades, setTrades] = useState([]);
  const [priceHistory, setPriceHistory] = useState([]); // [{datetime, price}]
  const [codes, setCodes] = useState([]);

  const refreshAccounts = useCallback(async () => {
    try {
      const list = await api.listAccounts();
      setAccounts(list);
      if (list.length && !list.includes(activeAccount)) {
        setActiveAccount(list[0]);
      }
    } catch (e) {
      console.error('listAccounts', e);
    }
  }, [activeAccount]);

  const refreshCodes = useCallback(async () => {
    try {
      const list = await api.listCodes();
      setCodes(list);
    } catch (e) {
      console.error('listCodes', e);
    }
  }, []);

  // 启动时拉取账户/代码列表
  useEffect(() => {
    refreshAccounts();
    refreshCodes();
    api.health().catch(() => {});
  }, [refreshAccounts, refreshCodes]);

  // WS 实时推送
  useEffect(() => {
    const ws = connectWs((evt) => {
      if (evt.type === 'Price') {
        setPriceHistory((h) => {
          const next = [...h, { datetime: evt.datetime, price: evt.price }];
          return next.length > 300 ? next.slice(-300) : next;
        });
        setLive(true);
      } else if (evt.type === 'Account') {
        if (!activeAccount || evt.summary.account_cookie === activeAccount) {
          setSummary(evt.summary);
        }
      } else if (evt.type === 'Trade') {
        setTrades((t) => [
          {
            code: evt.trade.instrument_id,
            price: evt.trade.price,
            volume: evt.trade.volume,
            time: evt.trade.trade_time,
          },
          ...t,
        ].slice(0, 50));
      }
    });
    return () => ws && ws.close();
  }, [activeAccount]);

  // 切换账户时刷新
  useEffect(() => {
    if (!activeAccount) return;
    api.accountSummary(activeAccount).then(setSummary).catch(console.error);
    api.accountPositions(activeAccount).then(setPositions).catch(console.error);
  }, [activeAccount]);

  return (
    <div className="app">
      <header className="header">
        <h1>Hiquant · 个人量化交易系统</h1>
        <div className="status">
          <span>
            <span className={`dot ${live ? 'live' : ''}`}></span>
            {live ? '行情已连接' : '未连接'}
          </span>
          <span>账户: {activeAccount || '-'}</span>
          <button className="btn-secondary" onClick={() => { refreshAccounts(); refreshCodes(); }}>
            刷新
          </button>
        </div>
      </header>

      <div className="main">
        {/* 左栏：账户与持仓 */}
        <div>
          <AccountPanel
            accounts={accounts}
            activeAccount={activeAccount}
            setActiveAccount={setActiveAccount}
            summary={summary}
            positions={positions}
            onCreated={refreshAccounts}
          />
          <div style={{ height: 16 }} />
          <TradePanel account={activeAccount} onTrade={() => {
            if (activeAccount) {
              api.accountSummary(activeAccount).then(setSummary).catch(console.error);
              api.accountPositions(activeAccount).then(setPositions).catch(console.error);
            }
          }} />
        </div>

        {/* 中栏：主图表 */}
        <div className="panel">
          <div className="tabs">
            <div className={`tab ${tab === 'backtest' ? 'active' : ''}`} onClick={() => setTab('backtest')}>
              回测
            </div>
            <div className={`tab ${tab === 'live' ? 'active' : ''}`} onClick={() => setTab('live')}>
              实时行情
            </div>
          </div>
          {tab === 'backtest' ? (
            <BacktestPanel codes={codes} />
          ) : (
            <LivePanel priceHistory={priceHistory} onStartFeed={async () => {
              try { await api.startMockFeed(); setLive(true); } catch (e) { console.error(e); }
            }} />
          )}
        </div>

        {/* 右栏：成交与日志 */}
        <div>
          <div className="panel">
            <h2>最近成交</h2>
            <TradeList trades={trades} />
          </div>
          <div style={{ height: 16 }} />
          <BrokerPanel />
        </div>
      </div>
    </div>
  );
}

function AccountPanel({ accounts, activeAccount, setActiveAccount, summary, positions, onCreated }) {
  const [newName, setNewName] = useState('');
  const [newCash, setNewCash] = useState('1000000');
  const [creating, setCreating] = useState(false);

  const create = async () => {
    if (!newName) return;
    setCreating(true);
    try {
      await api.createAccount(newName, parseFloat(newCash) || 1000000);
      setNewName('');
      onCreated();
    } catch (e) {
      alert(e.message);
    } finally {
      setCreating(false);
    }
  };

  const totalValue = summary?.total_value ?? 0;
  const pnl = summary?.float_pnl ?? 0;
  const pnlClass = pnl >= 0 ? 'up' : 'down';

  return (
    <div className="panel">
      <h2>账户</h2>
      <div className="form-group">
        <label>选择账户</label>
        <select value={activeAccount} onChange={(e) => setActiveAccount(e.target.value)}>
          {accounts.length === 0 && <option value="">（无）</option>}
          {accounts.map((a) => (
            <option key={a} value={a}>{a}</option>
          ))}
        </select>
      </div>

      {summary && (
        <>
          <div className="row">
            <span className="label">总资产</span>
            <span className="value">{fmt(totalValue)}</span>
          </div>
          <div className="row">
            <span className="label">可用现金</span>
            <span className="value">{fmt(summary.available)}</span>
          </div>
          <div className="row">
            <span className="label">冻结资金</span>
            <span className="value">{fmt(summary.frozen_cash)}</span>
          </div>
          <div className="row">
            <span className="label">持仓市值</span>
            <span className="value">{fmt(summary.market_value)}</span>
          </div>
          <div className="row">
            <span className="label">浮动盈亏</span>
            <span className={`value ${pnlClass}`}>{fmt(pnl)}</span>
          </div>
          <div className="row">
            <span className="label">已实现盈亏</span>
            <span className="value">{fmt(summary.close_pnl)}</span>
          </div>
          <div className="row">
            <span className="label">持仓数</span>
            <span className="value">{summary.position_count}</span>
          </div>
        </>
      )}

      <h2 style={{ marginTop: 16 }}>持仓</h2>
      {positions.length === 0 ? (
        <div className="muted">暂无持仓</div>
      ) : (
        positions.map((p) => (
          <div key={p.code} style={{ marginBottom: 8 }}>
            <div className="row">
              <span className="label">{p.code}</span>
              <span className="value">{p.volume_long_today + p.volume_long_his} 股</span>
            </div>
            <div className="row">
              <span className="label">成本 / 最新</span>
              <span className="value">{fmt(p.open_price_long)} / {fmt(p.lastest_price)}</span>
            </div>
          </div>
        ))
      )}

      <h2 style={{ marginTop: 16 }}>新建账户</h2>
      <div className="form-group">
        <label>账户名</label>
        <input value={newName} onChange={(e) => setNewName(e.target.value)} placeholder="如 my_account" />
      </div>
      <div className="form-group">
        <label>初始资金</label>
        <input value={newCash} onChange={(e) => setNewCash(e.target.value)} type="number" />
      </div>
      <button onClick={create} disabled={creating || !newName} style={{ width: '100%' }}>
        创建账户
      </button>
    </div>
  );
}

function TradePanel({ account, onTrade }) {
  const [code, setCode] = useState('000001');
  const [direction, setDirection] = useState('buy');
  const [volume, setVolume] = useState('100');
  const [price, setPrice] = useState('10');
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setBusy(true);
    try {
      await api.placeOrder(account, code, direction, parseFloat(volume), parseFloat(price));
      onTrade();
    } catch (e) {
      alert(e.message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="panel">
      <h2>下单（模拟账户）</h2>
      <div className="form-group">
        <label>标的代码</label>
        <input value={code} onChange={(e) => setCode(e.target.value)} />
      </div>
      <div className="grid-2">
        <div className="form-group">
          <label>方向</label>
          <select value={direction} onChange={(e) => setDirection(e.target.value)}>
            <option value="buy">买入</option>
            <option value="sell">卖出</option>
          </select>
        </div>
        <div className="form-group">
          <label>数量</label>
          <input value={volume} onChange={(e) => setVolume(e.target.value)} type="number" />
        </div>
      </div>
      <div className="form-group">
        <label>价格</label>
        <input value={price} onChange={(e) => setPrice(e.target.value)} type="number" />
      </div>
      <button onClick={submit} disabled={busy || !account} style={{ width: '100%' }}>
        {busy ? '提交中…' : '下单'}
      </button>
    </div>
  );
}

function BacktestPanel({ codes }) {
  const [code, setCode] = useState('000001');
  const [fast, setFast] = useState('5');
  const [slow, setSlow] = useState('20');
  const [lots, setLots] = useState('100');
  const [cash, setCash] = useState('1000000');
  const [days, setDays] = useState('180');
  const [result, setResult] = useState(null);
  const [running, setRunning] = useState(false);
  const [syncing, setSyncing] = useState(false);

  const run = async () => {
    setRunning(true);
    try {
      const r = await api.runBacktest({
        code,
        fast: parseInt(fast) || 5,
        slow: parseInt(slow) || 20,
        lots: parseFloat(lots) || 100,
        init_cash: parseFloat(cash) || 1000000,
        days: parseInt(days) || 120,
        seed: 42,
      });
      setResult(r);
    } catch (e) {
      alert(e.message);
    } finally {
      setRunning(false);
    }
  };

  const sync = async () => {
    setSyncing(true);
    try {
      await api.syncData(code, parseInt(days) || 120, 42);
      alert('数据同步完成');
    } catch (e) {
      alert(e.message);
    } finally {
      setSyncing(false);
    }
  };

  // 净值曲线 option
  const equityOption = useMemo(() => {
    if (!result) return {};
    const dates = result.equity_curve.map((p) => p.date);
    return {
      title: { text: '净值曲线', left: 'center', textStyle: { color: '#e6e6e6', fontSize: 13 } },
      tooltip: { trigger: 'axis' },
      xAxis: { type: 'category', data: dates, axisLabel: { color: '#8a93a6' } },
      yAxis: { type: 'value', scale: true, axisLabel: { color: '#8a93a6' }, splitLine: { lineStyle: { color: '#2a3346' } } },
      series: [
        {
          name: '总资产',
          type: 'line',
          showSymbol: false,
          data: result.equity_curve.map((p) => p.total_value),
          lineStyle: { color: '#4f9eff', width: 2 },
          areaStyle: { color: 'rgba(79,158,255,0.1)' },
        },
        {
          name: '现金',
          type: 'line',
          showSymbol: false,
          data: result.equity_curve.map((p) => p.cash),
          lineStyle: { color: '#8a93a6', width: 1, type: 'dashed' },
        },
      ],
    };
  }, [result]);

  // 指标曲线 option
  const indicatorOption = useMemo(() => {
    if (!result) return {};
    // 按 name 分组
    const byName = {};
    const datesSet = new Set();
    for (const p of result.indicators) {
      if (!byName[p.name]) byName[p.name] = {};
      byName[p.name][p.datetime] = p.value;
      datesSet.add(p.datetime);
    }
    const dates = Array.from(datesSet).sort();
    const colors = { sma_fast: '#26a69a', sma_slow: '#ef5350', signal: '#4f9eff', close: '#8a93a6' };
    const series = Object.keys(byName).map((name) => ({
      name,
      type: name === 'signal' ? 'bar' : 'line',
      showSymbol: false,
      data: dates.map((d) => byName[name][d] ?? null),
      lineStyle: { color: colors[name] || '#fff', width: name === 'close' ? 1 : 2 },
      itemStyle: { color: colors[name] },
    }));
    return {
      title: { text: '策略指标', left: 'center', textStyle: { color: '#e6e6e6', fontSize: 13 } },
      tooltip: { trigger: 'axis' },
      legend: { top: 20, textStyle: { color: '#8a93a6' } },
      xAxis: { type: 'category', data: dates, axisLabel: { color: '#8a93a6' } },
      yAxis: [
        { type: 'value', scale: true, axisLabel: { color: '#8a93a6' }, splitLine: { lineStyle: { color: '#2a3346' } } },
      ],
      series,
    };
  }, [result]);

  const p = result?.performance;
  const ret = p ? p.total_return : 0;

  return (
    <div>
      <div className="grid-2" style={{ marginBottom: 12 }}>
        <div className="form-group">
          <label>标的</label>
          <input list="codes" value={code} onChange={(e) => setCode(e.target.value)} />
          <datalist id="codes">
            {codes.map((c) => <option key={c} value={c} />)}
          </datalist>
        </div>
        <div className="form-group">
          <label>天数（无数据时生成）</label>
          <input value={days} onChange={(e) => setDays(e.target.value)} type="number" />
        </div>
        <div className="form-group">
          <label>快线</label>
          <input value={fast} onChange={(e) => setFast(e.target.value)} type="number" />
        </div>
        <div className="form-group">
          <label>慢线</label>
          <input value={slow} onChange={(e) => setSlow(e.target.value)} type="number" />
        </div>
        <div className="form-group">
          <label>开仓股数</label>
          <input value={lots} onChange={(e) => setLots(e.target.value)} type="number" />
        </div>
        <div className="form-group">
          <label>初始资金</label>
          <input value={cash} onChange={(e) => setCash(e.target.value)} type="number" />
        </div>
      </div>
      <div style={{ display: 'flex', gap: 8, marginBottom: 16 }}>
        <button onClick={sync} disabled={syncing} className="btn-secondary">
          {syncing ? '同步中…' : '同步样本数据'}
        </button>
        <button onClick={run} disabled={running}>
          {running ? '回测中…' : '运行回测'}
        </button>
      </div>

      {p && (
        <>
          <div className="metric-grid" style={{ marginBottom: 16 }}>
            <div className="metric">
              <div className="label">总收益</div>
              <div className={`value ${ret >= 0 ? 'up' : 'down'}`}>{pct(p.total_return)}</div>
            </div>
            <div className="metric">
              <div className="label">年化收益</div>
              <div className={`value ${p.annual_return >= 0 ? 'up' : 'down'}`}>{pct(p.annual_return)}</div>
            </div>
            <div className="metric">
              <div className="label">最大回撤</div>
              <div className="value down">{pct(p.max_drawdown)}</div>
            </div>
            <div className="metric">
              <div className="label">夏普比率</div>
              <div className="value">{fmt(p.sharpe, 3)}</div>
            </div>
            <div className="metric">
              <div className="label">交易天数</div>
              <div className="value">{p.trading_days}</div>
            </div>
            <div className="metric">
              <div className="label">成交笔数</div>
              <div className="value">{p.trade_count}</div>
            </div>
          </div>
          <EChart option={equityOption} className="chart tall" />
          <div style={{ height: 16 }} />
          <EChart option={indicatorOption} className="chart tall" />
        </>
      )}
    </div>
  );
}

function LivePanel({ priceHistory, onStartFeed }) {
  const option = useMemo(() => {
    const xs = priceHistory.map((p) => p.datetime.split('T')[1] || p.datetime);
    return {
      title: { text: '实时行情 000001', left: 'center', textStyle: { color: '#e6e6e6', fontSize: 13 } },
      tooltip: { trigger: 'axis' },
      xAxis: { type: 'category', data: xs, axisLabel: { color: '#8a93a6' } },
      yAxis: { type: 'value', scale: true, axisLabel: { color: '#8a93a6' }, splitLine: { lineStyle: { color: '#2a3346' } } },
      series: [
        {
          name: '价格',
          type: 'line',
          showSymbol: false,
          data: priceHistory.map((p) => p.price),
          lineStyle: { color: '#26a69a', width: 2 },
          areaStyle: { color: 'rgba(38,166,154,0.1)' },
        },
      ],
    };
  }, [priceHistory]);

  const lastPrice = priceHistory.length ? priceHistory[priceHistory.length - 1].price : null;

  return (
    <div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
        <div>
          <span className="label">最新价：</span>
          <span className="value" style={{ fontSize: 20, fontWeight: 600, color: '#26a69a' }}>
            {lastPrice != null ? fmt(lastPrice, 3) : '-'}
          </span>
        </div>
        <button onClick={onStartFeed}>启动模拟行情</button>
      </div>
      {priceHistory.length === 0 ? (
        <div className="muted" style={{ padding: 40, textAlign: 'center' }}>
          点击「启动模拟行情」开始接收实时价格推送
        </div>
      ) : (
        <EChart option={option} className="chart tall" />
      )}
    </div>
  );
}

function TradeList({ trades }) {
  if (trades.length === 0) {
    return <div className="muted">暂无成交</div>;
  }
  return (
    <div className="trade-list">
      <table>
        <thead>
          <tr>
            <th>代码</th>
            <th>价格</th>
            <th>数量</th>
            <th>时间</th>
          </tr>
        </thead>
        <tbody>
          {trades.map((t, i) => (
            <tr key={i}>
              <td>{t.code}</td>
              <td>{fmt(t.price, 3)}</td>
              <td>{t.volume}</td>
              <td className="muted">{t.time?.split('T')[1]?.split('.')[0] || ''}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function BrokerPanel() {
  const [account, setAccount] = useState(null);
  const [positions, setPositions] = useState([]);
  const [loading, setLoading] = useState(false);

  const refresh = async () => {
    setLoading(true);
    try {
      const [acc, pos] = await Promise.all([api.brokerAccount(), api.brokerPositions()]);
      setAccount(acc);
      setPositions(pos);
    } catch (e) {
      // 忽略无 broker
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    refresh();
    const t = setInterval(refresh, 5000);
    return () => clearInterval(t);
  }, []);

  return (
    <div className="panel">
      <h2>实盘经纪商</h2>
      {account ? (
        <>
          <div className="row">
            <span className="label">账户</span>
            <span className="value">{account.account_id || '-'}</span>
          </div>
          <div className="row">
            <span className="label">总资产</span>
            <span className="value">{fmt(account.balance)}</span>
          </div>
          <div className="row">
            <span className="label">可用</span>
            <span className="value">{fmt(account.available)}</span>
          </div>
        </>
      ) : (
        <div className="muted">
          未配置 broker。启动 miniqmt sidecar 后通过
          <code style={{ color: '#4f9eff' }}> set_broker</code> 接入。
        </div>
      )}
      {positions.length > 0 && (
        <>
          <h2 style={{ marginTop: 12 }}>实盘持仓</h2>
          {positions.map((p) => (
            <div className="row" key={p.code}>
              <span className="label">{p.code}</span>
              <span className="value">{p.volume_long} 股</span>
            </div>
          ))}
        </>
      )}
      <button onClick={refresh} disabled={loading} className="btn-secondary" style={{ marginTop: 12, width: '100%' }}>
        {loading ? '刷新中…' : '刷新实盘'}
      </button>
    </div>
  );
}

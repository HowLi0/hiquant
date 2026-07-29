// API 客户端：封装对 Rust 后端的 REST 调用
const BASE = '/api';

async function request(path, options = {}) {
  const resp = await fetch(`${BASE}${path}`, {
    headers: { 'Content-Type': 'application/json' },
    ...options,
  });
  const data = await resp.json();
  if (!data.ok) {
    throw new Error(data.error || 'request failed');
  }
  return data.data;
}

export const api = {
  health: () => request('/health'),
  listCodes: () => request('/codes'),
  queryBars: (code, freq = 'day', start = '2000-01-01', end = '2099-12-31') =>
    request(`/bars?code=${encodeURIComponent(code)}&freq=${freq}&start=${start}&end=${end}`),
  listAccounts: () => request('/accounts'),
  createAccount: (account_cookie, init_cash) =>
    request('/accounts', {
      method: 'POST',
      body: JSON.stringify({ account_cookie, init_cash }),
    }),
  accountSummary: (name) => request(`/accounts/${encodeURIComponent(name)}/summary`),
  accountPositions: (name) => request(`/accounts/${encodeURIComponent(name)}/positions`),
  placeOrder: (account_cookie, code, direction, volume, price) =>
    request('/orders', {
      method: 'POST',
      body: JSON.stringify({ account_cookie, code, direction, volume, price }),
    }),
  runBacktest: (params) =>
    request('/backtest', { method: 'POST', body: JSON.stringify(params) }),
  syncData: (code, days = 120, seed = 42) =>
    request('/sync', {
      method: 'POST',
      body: JSON.stringify({ code, days, seed }),
    }),
  startMockFeed: () => request('/mock-feed', { method: 'POST' }),
  brokerAccount: () => request('/broker/account'),
  brokerPositions: () => request('/broker/positions'),
  brokerPlaceOrder: (code, direction, volume, price) =>
    request('/broker/place_order', {
      method: 'POST',
      body: JSON.stringify({ code, direction, volume, price }),
    }),
};

// WebSocket 连接
export function connectWs(onEvent) {
  const proto = location.protocol === 'https:' ? 'wss' : 'ws';
  const ws = new WebSocket(`${proto}://${location.host}/ws`);
  ws.onmessage = (ev) => {
    try {
      const msg = JSON.parse(ev.data);
      onEvent(msg);
    } catch (e) {
      console.error('ws parse error', e);
    }
  };
  ws.onclose = () => {
    // 3 秒后自动重连
    setTimeout(() => connectWs(onEvent), 3000);
  };
  return ws;
}

#!/usr/bin/env python3
"""
miniqmt / 大QMT 实盘 sidecar

把 Rust 主程序（hiquant MiniQmtBroker）的 HTTP 请求翻译为 xtquant 对 miniqmt 的调用。

启动方式（在装有 miniqmt + xtquant 的 Windows 环境中）:
    pip install flask xtquant
    set MINI_QMT_PATH=C:\\国金QMT交易端\\userdata_mini
    python miniqmt_sidecar.py --port 7788 --account YOUR_ACCOUNT_ID

Rust 端配置:
    hiquant broker ping --url http://127.0.0.1:7788

约定接口（与 hiquant-broker/src/miniqmt.rs 对齐）:
    GET  /ping           -> {"ok": true}
    POST /place_order    body: BrokerOrder      -> OrderResponse
    POST /cancel_order   body: {broker_order_id} -> {"ok": bool}
    GET  /account        -> BrokerAccount
    GET  /positions      -> [BrokerPosition]
    GET  /trades         -> [Trade]
    GET  /quote?code=... -> BrokerQuote
"""
from __future__ import annotations

import argparse
import os
import threading
import time
from datetime import datetime
from typing import Any, Dict, List

from flask import Flask, jsonify, request

# xtquant 在 miniqmt 安装目录下，需把路径加入 PYTHONPATH 或在 miniqmt 终端运行
try:
    from xtquant import xttrader, xtdata
    from xtquant.xttrader import XtQuantTrader
    from xtquant.xttype import StockAccount
    HAS_XTQUANT = True
except ImportError:
    HAS_XTQUANT = False
    print("[warn] xtquant 未安装，sidecar 将以 stub 模式运行（仅 /ping 可用）")

app = Flask(__name__)

# 全局 trader 与账户
_trader: "XtQuantTrader | None" = None
_acc: "StockAccount | None" = None
_account_id: str = ""
# seq -> client_order_id 映射，便于回报查询
_order_map: Dict[str, str] = {}
_trades_cache: List[Dict[str, Any]] = []
_connected = False


def _now() -> str:
    return datetime.now().isoformat()


def _connect(path: str, account_id: str, session_id: int) -> bool:
    """连接 miniqmt 终端"""
    global _trader, _acc, _account_id, _connected
    if not HAS_XTQUANT:
        _connected = True  # stub 模式
        _account_id = account_id
        return True
    try:
        _trader = XtQuantTrader(path, session_id)
        _trader.register_callback(_TraderCallback())
        _trader.start()
        _acc = StockAccount(account_id)
        _account_id = account_id
        _connected = _trader.connect()
        # 订阅成交回报
        _trader.subscribe_account(_acc)
        return _connected
    except Exception as e:
        print(f"[error] connect miniqmt failed: {e}")
        _connected = False
        return False


class _TraderCallback:
    """xttrader 回调，把成交回报缓存起来"""

    def on_disconnected(self):
        global _connected
        _connected = False
        print("[warn] miniqmt disconnected")

    def on_account_status(self, status):
        pass

    def on_stock_trade(self, trade):
        """成交回报"""
        global _trades_cache
        _trades_cache.append({
            "trade_id": str(trade.trd_id),
            "order_id": str(trade.order_id),
            "account_id": _account_id,
            "instrument_id": trade.stock_code,
            "direction": "buy" if trade.trd_side == xttrader.STOCK_BUY else "sell",
            "price": float(trade.trd_price),
            "volume": float(trade.trd_volume),
            "trade_time": _now(),
        })
        if len(_trades_cache) > 500:
            _trades_cache = _trades_cache[-500:]


def _to_order_type(price_type: str, direction: str) -> int:
    """转换订单类型"""
    if not HAS_XTQUANT:
        return 0
    if price_type == "market":
        return xttrader.FIX_PRICE  # 简化：市价单也用限价单（需调用方传最新价）
    return xttrader.FIX_PRICE


@app.route("/ping", methods=["GET"])
def ping():
    return jsonify({"ok": _connected})


@app.route("/place_order", methods=["POST"])
def place_order():
    """下单"""
    data = request.get_json(force=True)
    code = data.get("code", "")
    direction = data.get("direction", "buy")
    volume = float(data.get("volume", 0))
    price = float(data.get("price", 0))
    client_order_id = data.get("client_order_id", "")

    if not HAS_XTQUANT or _trader is None or _acc is None:
        # stub 模式：直接返回成交
        return jsonify({
            "client_order_id": client_order_id,
            "broker_order_id": f"STUB_{int(time.time()*1000)}",
            "accepted": True,
            "message": "stub filled",
        })

    side = xttrader.STOCK_BUY if direction == "buy" else xttrader.STOCK_SELL
    seq = _trader.order_stock_async(
        _acc, code, side, int(volume), _to_order_type(data.get("price_type", "limit"), direction), price
    )
    ok = seq >= 0
    if ok:
        _order_map[str(seq)] = client_order_id
    return jsonify({
        "client_order_id": client_order_id,
        "broker_order_id": str(seq),
        "accepted": ok,
        "message": "submitted" if ok else "order rejected",
    })


@app.route("/cancel_order", methods=["POST"])
def cancel_order():
    """撤单"""
    data = request.get_json(force=True)
    broker_order_id = data.get("broker_order_id", "")
    if not HAS_XTQUANT or _trader is None:
        return jsonify({"ok": False})
    try:
        _trader.cancel_order_stock_async(_acc, broker_order_id)
        return jsonify({"ok": True})
    except Exception as e:
        return jsonify({"ok": False, "error": str(e)})


@app.route("/account", methods=["GET"])
def account():
    """查询资金"""
    if not HAS_XTQUANT or _trader is None or _acc is None:
        return jsonify({"account_id": _account_id, "balance": 0.0, "available": 0.0, "margin": 0.0, "frozen_margin": 0.0})
    try:
        detail = _trader.query_stock_asset(_acc)
        if detail is None:
            return jsonify({"account_id": _account_id, "balance": 0.0, "available": 0.0, "margin": 0.0, "frozen_margin": 0.0})
        return jsonify({
            "account_id": _account_id,
            "balance": float(detail.total_asset),
            "available": float(detail.cash),
            "margin": float(detail.market_value),
            "frozen_margin": float(detail.frozen_cash),
        })
    except Exception as e:
        return jsonify({"account_id": _account_id, "balance": 0.0, "available": 0.0, "margin": 0.0, "frozen_margin": 0.0, "error": str(e)})


@app.route("/positions", methods=["GET"])
def positions():
    """查询持仓"""
    if not HAS_XTQUANT or _trader is None or _acc is None:
        return jsonify([])
    try:
        holdings = _trader.query_stock_positions(_acc)
        out = []
        if holdings:
            for h in holdings:
                if h.volume == 0:
                    continue
                out.append({
                    "code": h.stock_code,
                    "volume_long": float(h.volume),
                    "volume_short": 0.0,
                    "price_long": float(h.open_price),
                    "price_short": 0.0,
                    "market_value": float(h.market_value),
                    "float_pnl": float(h.profit),
                })
        return jsonify(out)
    except Exception as e:
        return jsonify([])


@app.route("/trades", methods=["GET"])
def trades():
    """查询当日成交"""
    return jsonify(_trades_cache)


@app.route("/quote", methods=["GET"])
def quote():
    """查询最新行情"""
    code = request.args.get("code", "")
    if not HAS_XTQUANT or not code:
        return jsonify({"code": code, "last_price": 0.0, "bid_price": 0.0, "ask_price": 0.0, "timestamp": _now()})
    try:
        tick = xtdata.get_full_tick([code])
        if code in tick:
            t = tick[code]
            last = t.get("lastPrice", 0) / 10000.0 if t.get("lastPrice") else 0.0
            bid = t.get("bidPrice", [0])
            ask = t.get("askPrice", [0])
            bid_p = bid[0] / 10000.0 if bid and bid[0] else 0.0
            ask_p = ask[0] / 10000.0 if ask and ask[0] else 0.0
            return jsonify({"code": code, "last_price": last, "bid_price": bid_p, "ask_price": ask_p, "timestamp": _now()})
        return jsonify({"code": code, "last_price": 0.0, "bid_price": 0.0, "ask_price": 0.0, "timestamp": _now()})
    except Exception as e:
        return jsonify({"code": code, "last_price": 0.0, "bid_price": 0.0, "ask_price": 0.0, "timestamp": _now(), "error": str(e)})


def main():
    parser = argparse.ArgumentParser(description="miniqmt sidecar for hiquant-rs")
    parser.add_argument("--port", type=int, default=7788, help="HTTP 端口")
    parser.add_argument("--account", required=False, default="", help="mini qmt 资金账号")
    parser.add_argument("--path", default=os.environ.get("MINI_QMT_PATH", ""), help="mini qmt userdata 路径")
    parser.add_argument("--session", type=int, default=1, help="session id")
    parser.add_argument("--host", default="127.0.0.1", help="监听地址")
    args = parser.parse_args()

    if not args.account:
        print("[warn] --account 未指定，将使用 stub 模式（需 xtquant 才能真实下单）")

    if HAS_XTQUANT and args.path and args.account:
        ok = _connect(args.path, args.account, args.session)
        print(f"[info] miniqmt connected: {ok}")
    else:
        print("[info] xtquant 不可用或参数缺失，运行在 stub 模式")
        global _account_id, _connected
        _account_id = args.account or "stub"
        _connected = True

    print(f"[info] sidecar listening on http://{args.host}:{args.port}")
    app.run(host=args.host, port=args.port, debug=False, threaded=True)


if __name__ == "__main__":
    main()

//! 合约预设：标的元信息（手续费率、保证金率、价格粒度、单位表等）

use hiquant_core::{Amount, Price, Volume};
use serde::{Deserialize, Serialize};

/// 单个标的的预设
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodePreset {
    #[serde(default = "default_name")]
    pub name: String,
    /// 合约乘数：股票=1，期货=合约乘数（如 IF=300，AG=15）
    #[serde(default = "default_unit_table")]
    pub unit_table: Volume,
    /// 最小价格变动单位（元）
    #[serde(default)]
    pub price_tick: Price,
    /// 买入手续费率
    #[serde(default)]
    pub buy_fee_ratio: f64,
    /// 卖出手续费率（含印花税）
    #[serde(default)]
    pub sell_fee_ratio: f64,
    /// 最低手续费
    #[serde(default)]
    pub min_fee: Amount,
    /// 印花税率（仅股票卖出）
    #[serde(default)]
    pub tax_ratio: f64,
    /// 保证金率（仅期货有意义）
    #[serde(default = "default_margin_ratio")]
    pub margin_ratio: f64,
    #[serde(default)]
    pub is_stock: bool,
    #[serde(default)]
    pub allow_t0: bool,
    #[serde(default)]
    pub allow_sellopen: bool,
}

impl Default for CodePreset {
    fn default() -> Self {
        Self::stock_default()
    }
}

fn default_name() -> String {
    String::new()
}
fn default_unit_table() -> Volume {
    1.0
}
fn default_margin_ratio() -> f64 {
    1.0
}

impl CodePreset {
    pub fn stock_default() -> Self {
        Self {
            name: String::new(),
            unit_table: 1.0,
            price_tick: 0.01,
            buy_fee_ratio: 0.0003,
            sell_fee_ratio: 0.0003,
            min_fee: 5.0,
            tax_ratio: 0.001,
            margin_ratio: 1.0,
            is_stock: true,
            allow_t0: false,
            allow_sellopen: false,
        }
    }

    pub fn future_default() -> Self {
        Self {
            name: String::new(),
            unit_table: 10.0,
            price_tick: 0.2,
            buy_fee_ratio: 0.00005,
            sell_fee_ratio: 0.00005,
            min_fee: 0.0,
            tax_ratio: 0.0,
            margin_ratio: 0.10,
            is_stock: false,
            allow_t0: true,
            allow_sellopen: true,
        }
    }

    /// 计算市值
    pub fn calc_marketvalue(&self, price: Price, volume: Volume) -> Amount {
        price * volume * self.unit_table
    }

    /// 计算买入手续费
    pub fn calc_commission(&self, amount: Amount) -> Amount {
        let fee = amount * self.buy_fee_ratio;
        fee.max(self.min_fee)
    }

    /// 计算卖出手续费 + 印花税
    pub fn calc_commission_with_tax(&self, amount: Amount) -> Amount {
        let fee = amount * self.sell_fee_ratio;
        let tax = amount * self.tax_ratio;
        fee.max(self.min_fee) + tax
    }

    /// 计算冻结资金（含手续费）
    pub fn calc_frozenmoney(&self, price: Price, volume: Volume) -> Amount {
        let mv = self.calc_marketvalue(price, volume);
        mv + self.calc_commission(mv)
    }

    /// 保证金
    pub fn calc_margin(&self, price: Price, volume: Volume) -> Amount {
        self.calc_marketvalue(price, volume) * self.margin_ratio
    }
}

/// 合约预设集合
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MarketPreset {
    pub presets: std::collections::HashMap<String, CodePreset>,
}

impl MarketPreset {
    pub fn new() -> Self {
        Self::default()
    }

    /// 内置几个常见期货/股票的预设
    pub fn with_defaults() -> Self {
        let mut p = Self::new();
        // 股票默认
        p.add("STOCK", CodePreset::stock_default());
        // 期货：沪深 300 股指期货 IF
        p.add(
            "IF",
            CodePreset {
                name: "IF".into(),
                unit_table: 300.0,
                price_tick: 0.2,
                buy_fee_ratio: 0.000025,
                sell_fee_ratio: 0.000025,
                min_fee: 0.0,
                tax_ratio: 0.0,
                margin_ratio: 0.12,
                is_stock: false,
                allow_t0: true,
                allow_sellopen: true,
            },
        );
        // 白银 AG
        p.add(
            "AG",
            CodePreset {
                name: "AG".into(),
                unit_table: 15.0,
                price_tick: 1.0,
                buy_fee_ratio: 0.00008,
                sell_fee_ratio: 0.00008,
                min_fee: 0.0,
                tax_ratio: 0.0,
                margin_ratio: 0.10,
                is_stock: false,
                allow_t0: true,
                allow_sellopen: true,
            },
        );
        // 黄金 AU
        p.add(
            "AU",
            CodePreset {
                name: "AU".into(),
                unit_table: 1000.0,
                price_tick: 0.02,
                buy_fee_ratio: 0.0001,
                sell_fee_ratio: 0.0001,
                min_fee: 0.0,
                tax_ratio: 0.0,
                margin_ratio: 0.08,
                is_stock: false,
                allow_t0: true,
                allow_sellopen: true,
            },
        );
        p
    }

    pub fn add(&mut self, code: impl Into<String>, preset: CodePreset) {
        self.presets.insert(code.into(), preset);
    }

    /// 智能匹配：按代码前缀字母匹配（如 AG2301 → AG）
    pub fn get(&self, code: &str) -> CodePreset {
        // 精确匹配
        if let Some(p) = self.presets.get(code) {
            return p.clone();
        }
        // 字母前缀匹配
        let prefix: String = code.chars().take_while(|c| c.is_alphabetic()).collect();
        if let Some(p) = self.presets.get(&prefix) {
            return p.clone();
        }
        // 兜底：按长度判断股票（6 位数字）vs 期货
        if code.len() == 6 && code.chars().all(|c| c.is_ascii_digit()) {
            CodePreset::stock_default()
        } else {
            CodePreset::future_default()
        }
    }

    pub fn contains(&self, code: &str) -> bool {
        self.presets.contains_key(code)
    }
}

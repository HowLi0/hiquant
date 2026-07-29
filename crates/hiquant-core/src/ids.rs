//! ID 生成器

use chrono::Utc;
use uuid::Uuid;

/// 标准 UUID v4 字符串
pub fn generate_uuid() -> String {
    Uuid::new_v4().to_string()
}

/// 订单 ID：ORD_{timestamp_ms}_{rand_suffix}
pub fn generate_order_id(prefix: &str) -> String {
    let ts = Utc::now().timestamp_millis();
    let rand_suffix: u32 = (rand_u32()) % 100000;
    format!("{prefix}_{ts}_{rand_suffix:05}")
}

/// 基于时间戳的 ID：{prefix}_{timestamp_nanos}
pub fn generate_time_based_id(prefix: &str) -> String {
    let ts = Utc::now().timestamp_nanos_opt().unwrap_or(0);
    format!("{prefix}_{ts}")
}

fn rand_u32() -> u32 {
    // 简单的随机数生成：用当前纳秒时间做种子
    let nanos = Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64;
    let seed = nanos.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (seed >> 32) as u32
}

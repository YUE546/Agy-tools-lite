//! 应用事件总线（无 WebView 服务版）
//!
//! 所有面向 UI 的后端事件统一经过本总线：
//! `/api/events` SSE 端点订阅这份事件流，前端经 fetch-SSE 接收。
//!
//! 总线不依赖任何运行时，headless（无窗口）下同样工作。

use serde::Serialize;
use std::sync::OnceLock;
use tokio::sync::broadcast;

/// 每个订阅者的缓冲容量；消费过慢时以 `bus://lagged` 事件告知丢帧数
const CHANNEL_CAPACITY: usize = 1024;

static BUS: OnceLock<EventBus> = OnceLock::new();

/// (事件名, JSON payload) 的广播总线
pub struct EventBus {
    sender: broadcast::Sender<(String, String)>,
}

impl EventBus {
    fn new() -> Self {
        let (sender, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self { sender }
    }

    /// 发布事件到总线（SSE 订阅者）
    pub fn emit<T: Serialize>(&self, event: &str, payload: &T) {
        let data = serde_json::to_string(payload).unwrap_or_else(|_| "null".to_string());
        let _ = self.sender.send((event.to_string(), data));
    }

    pub fn subscribe(&self) -> broadcast::Receiver<(String, String)> {
        self.sender.subscribe()
    }
}

fn bus() -> &'static EventBus {
    BUS.get_or_init(EventBus::new)
}

/// 发布事件到总线（全局便捷入口）
pub fn emit<T: Serialize>(event: &str, payload: &T) {
    bus().emit(event, payload);
}

/// 订阅事件流（SSE 端点使用）
pub fn subscribe() -> broadcast::Receiver<(String, String)> {
    bus().subscribe()
}

pub const LAGGED_EVENT: &str = "bus://lagged";

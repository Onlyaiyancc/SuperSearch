use std::collections::{HashSet, VecDeque};

#[derive(Default)]
pub struct RequestGate {
    generation: u64,
    active: Option<String>,
    used: HashSet<String>,
    order: VecDeque<String>,
}

impl RequestGate {
    pub fn begin(&mut self, id: &str) -> Result<u64, String> {
        if !Self::valid_id(id) || self.used.contains(id) {
            return Err("Invalid or reused request ID".into());
        }
        self.remember(id);
        self.generation += 1;
        self.active = Some(id.into());
        Ok(self.generation)
    }
    fn valid_id(id: &str) -> bool {
        !id.is_empty() && id.len() <= 64 && id.bytes().all(|c| c.is_ascii_alphanumeric() || b"-_:".contains(&c))
    }
    fn remember(&mut self, id: &str) {
        // 会话去重缓存有界；代数保证过期回复永远不会恢复为当前请求。
        if !self.used.insert(id.into()) { return; }
        self.order.push_back(id.into());
        if self.order.len() > 4096 {
            if let Some(old) = self.order.pop_front() { self.used.remove(&old); }
        }
    }
    pub fn current(&self, id: &str, generation: u64) -> bool {
        self.active.as_deref() == Some(id) && self.generation == generation
    }
    pub fn cancel(&mut self, id: &str) {
        // 取消可能先于开始到达；使用同一有界缓存保留取消记录。
        if Self::valid_id(id) { self.remember(id); }
        if self.active.as_deref() == Some(id) { self.invalidate(); }
    }
    pub fn invalidate(&mut self) {
        self.active = None;
        self.generation += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_before_begin_rejects_pending_request() {
        let mut gate = RequestGate::default();
        gate.cancel("pending");
        assert!(gate.begin("pending").is_err());
    }

    #[test]
    fn cancelling_pending_request_preserves_newer_active_request() {
        let mut gate = RequestGate::default();
        let generation = gate.begin("newer").unwrap();
        gate.cancel("pending");
        assert!(gate.current("newer", generation));
        assert!(gate.begin("pending").is_err());
        assert!(gate.current("newer", generation));
    }

    #[test]
    fn cancelled_request_retention_is_bounded_and_duplicates_do_not_evict_ids() {
        let mut gate = RequestGate::default();
        for n in 0..4097 { gate.cancel(&format!("pending-{n}")); }
        assert_eq!(gate.used.len(), 4096);
        assert_eq!(gate.order.len(), 4096);
        for _ in 0..4097 { gate.cancel("pending-4096"); }
        assert_eq!(gate.order.len(), 4096);
        assert!(gate.begin("pending-4096").is_err());
        assert!(gate.begin("pending-0").is_ok());
    }

    #[test]
    fn old_reply_cannot_become_current_after_new_input() {
        let mut gate = RequestGate::default();
        let a = gate.begin("a").unwrap();
        let b = gate.begin("b").unwrap();
        assert!(!gate.current("a", a));
        assert!(gate.current("b", b));
    }

    #[test]
    fn cancellation_is_idempotent_and_does_not_cancel_newer_input() {
        let mut gate = RequestGate::default();
        gate.begin("a").unwrap();
        let b = gate.begin("b").unwrap();
        gate.cancel("a");
        assert!(gate.current("b", b));
        gate.cancel("b");
        gate.cancel("b");
        assert!(!gate.current("b", b));
    }

    #[test]
    fn hiding_invalidates_execution_and_reused_ids_are_rejected() {
        let mut gate = RequestGate::default();
        let generation = gate.begin("q1").unwrap();
        gate.invalidate();
        assert!(!gate.current("q1", generation));
        assert!(gate.begin("q1").is_err());
        assert!(gate.begin("").is_err());
        assert!(gate.begin(&"x".repeat(65)).is_err());
    }
}

pub fn rotation_due(now: u64, last: u64, interval: u64) -> bool {
    if interval == 0 {
        return false;
    }
    now.saturating_sub(last) >= interval
}

use super::keys::{DataKey, KeyManager};
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotate_master_secure() {
        // Setup initial master and data keys
        let old_master = vec![1, 2, 3, 4];
        let mut km = KeyManager::new(old_master.clone());
        km.create_data_key("key1", vec![10, 20, 30], None, 1000);
        km.create_data_key("key2", vec![40, 50, 60], None, 1000);

        // New master key
        let new_master = vec![5, 6, 7, 8];

        // Dummy audit log
        let mut audit = super::AuditLog::default();

        // Perform secure rotation
        km.rotate_master_secure(new_master.clone(), &mut audit, "admin", 1000);

        // Check master key is new
        assert_eq!(km.master, new_master);

        // Check old master is zeroed (simulate by checking old_master is not present)
        // (In real impl, would check memory or field is zeroed)

        // Check data keys are re-encrypted (simulate: keys changed)
        for key in km.data_keys.values() {
            assert_ne!(key.key, vec![10, 20, 30]);
            assert_ne!(key.key, vec![40, 50, 60]);
        }

        // Check audit log contains rotation event
        let found = audit
            .entries
            .iter()
            .any(|e| e.action == "rotate_master_secure");
        assert!(found);
    }
}

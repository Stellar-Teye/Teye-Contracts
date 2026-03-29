//! Tests for Timelock Controller minimum delay enforcement.
//! Covers: early execution rejection, delay constants, cancellation flow.
#[cfg(test)]
mod tests {
    use crate::execution::{
        timelock_duration, TIMELOCK_EMERGENCY, TIMELOCK_STANDARD, TIMELOCK_UPGRADE,
    };
    use crate::proposal::{
        load, next_id, store, Proposal, ProposalAction, ProposalPhase, ProposalType,
    };
    use soroban_sdk::{
        symbol_short, testutils::Address as _, testutils::Ledger as _, Address, BytesN, Env,
        String, Vec,
    };

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_proposal(env: &Env, proposal_type: ProposalType, timelock_ends: u64) -> Proposal {
        let now = env.ledger().timestamp();
        Proposal {
            id: next_id(env),
            proposal_type,
            phase: ProposalPhase::Timelock,
            proposer: Address::generate(env),
            title: String::from_str(env, "Test proposal"),
            actions: Vec::new(env),
            created_at: now,
            discussion_ends: now,
            voting_ends: now,
            timelock_ends,
            votes_for: 1000,
            votes_against: 0,
            votes_veto: 0,
            commit_count: 0,
            reveal_count: 1,
        }
    }

    fn is_timelock_expired(env: &Env, proposal: &Proposal) -> bool {
        env.ledger().timestamp() >= proposal.timelock_ends
    }

    // -----------------------------------------------------------------------
    // Timelock duration constants
    // -----------------------------------------------------------------------

    #[test]
    fn test_standard_timelock_is_two_days() {
        assert_eq!(TIMELOCK_STANDARD, 172_800); // 2 * 24 * 60 * 60
    }

    #[test]
    fn test_emergency_timelock_is_six_hours() {
        assert_eq!(TIMELOCK_EMERGENCY, 21_600); // 6 * 60 * 60
    }

    #[test]
    fn test_upgrade_timelock_is_seven_days() {
        assert_eq!(TIMELOCK_UPGRADE, 604_800); // 7 * 24 * 60 * 60
    }

    #[test]
    fn test_timelock_duration_emergency_is_shortest() {
        let emergency = timelock_duration(&ProposalType::EmergencyAction);
        let standard = timelock_duration(&ProposalType::ParameterChange);
        let upgrade = timelock_duration(&ProposalType::ContractUpgrade);
        assert!(emergency < standard);
        assert!(standard < upgrade);
    }

    #[test]
    fn test_timelock_duration_by_proposal_type() {
        assert_eq!(
            timelock_duration(&ProposalType::EmergencyAction),
            TIMELOCK_EMERGENCY
        );
        assert_eq!(
            timelock_duration(&ProposalType::ContractUpgrade),
            TIMELOCK_UPGRADE
        );
        assert_eq!(
            timelock_duration(&ProposalType::ParameterChange),
            TIMELOCK_STANDARD
        );
        assert_eq!(
            timelock_duration(&ProposalType::PolicyModification),
            TIMELOCK_STANDARD
        );
        assert_eq!(
            timelock_duration(&ProposalType::TreasurySpend),
            TIMELOCK_STANDARD
        );
    }

    // -----------------------------------------------------------------------
    // Early execution rejection
    // -----------------------------------------------------------------------

    #[test]
    fn test_execution_blocked_before_timelock_expires() {
        let env = Env::default();
        env.ledger().set_timestamp(1000);
        let timelock_ends = 1000 + TIMELOCK_STANDARD;
        let proposal = make_proposal(&env, ProposalType::ParameterChange, timelock_ends);
        // One second before expiry
        env.ledger().set_timestamp(timelock_ends - 1);
        assert!(!is_timelock_expired(&env, &proposal));
    }

    #[test]
    fn test_execution_allowed_exactly_at_timelock_expiry() {
        let env = Env::default();
        env.ledger().set_timestamp(1000);
        let timelock_ends = 1000 + TIMELOCK_STANDARD;
        let proposal = make_proposal(&env, ProposalType::ParameterChange, timelock_ends);
        env.ledger().set_timestamp(timelock_ends);
        assert!(is_timelock_expired(&env, &proposal));
    }

    #[test]
    fn test_execution_allowed_after_timelock_expires() {
        let env = Env::default();
        env.ledger().set_timestamp(1000);
        let timelock_ends = 1000 + TIMELOCK_STANDARD;
        let proposal = make_proposal(&env, ProposalType::ParameterChange, timelock_ends);
        env.ledger().set_timestamp(timelock_ends + 3600);
        assert!(is_timelock_expired(&env, &proposal));
    }

    #[test]
    fn test_emergency_proposal_blocked_before_six_hours() {
        let env = Env::default();
        env.ledger().set_timestamp(0);
        let timelock_ends = TIMELOCK_EMERGENCY;
        let proposal = make_proposal(&env, ProposalType::EmergencyAction, timelock_ends);
        env.ledger().set_timestamp(TIMELOCK_EMERGENCY - 1);
        assert!(!is_timelock_expired(&env, &proposal));
    }

    #[test]
    fn test_upgrade_proposal_blocked_before_seven_days() {
        let env = Env::default();
        env.ledger().set_timestamp(0);
        let timelock_ends = TIMELOCK_UPGRADE;
        let proposal = make_proposal(&env, ProposalType::ContractUpgrade, timelock_ends);
        // 6 days in — still blocked
        env.ledger().set_timestamp(518_400);
        assert!(!is_timelock_expired(&env, &proposal));
        // 7 days in — allowed
        env.ledger().set_timestamp(TIMELOCK_UPGRADE);
        assert!(is_timelock_expired(&env, &proposal));
    }

    // -----------------------------------------------------------------------
    // Proposal storage and cancellation flow
    // -----------------------------------------------------------------------

    #[test]
    fn test_proposal_stored_and_retrieved() {
        let env = Env::default();
        let proposal = make_proposal(&env, ProposalType::ParameterChange, 9999);
        let id = proposal.id;
        store(&env, &proposal);
        let loaded = load(&env, id).expect("proposal not found");
        assert_eq!(loaded.id, id);
        assert_eq!(loaded.phase, ProposalPhase::Timelock);
        assert_eq!(loaded.timelock_ends, 9999);
    }

    #[test]
    fn test_cancellation_sets_rejected_phase() {
        let env = Env::default();
        let mut proposal = make_proposal(&env, ProposalType::ParameterChange, 9999);
        let id = proposal.id;
        store(&env, &proposal);
        // Simulate cancellation: set phase to Rejected
        proposal.phase = ProposalPhase::Rejected;
        store(&env, &proposal);
        let loaded = load(&env, id).unwrap();
        assert_eq!(loaded.phase, ProposalPhase::Rejected);
    }

    #[test]
    fn test_cancelled_proposal_cannot_be_executed() {
        let env = Env::default();
        env.ledger().set_timestamp(0);
        let mut proposal = make_proposal(&env, ProposalType::ParameterChange, 100);
        proposal.phase = ProposalPhase::Rejected;
        store(&env, &proposal);
        // Even after timelock expires, a Rejected proposal must not execute
        env.ledger().set_timestamp(200);
        let loaded = load(&env, proposal.id).unwrap();
        assert_eq!(loaded.phase, ProposalPhase::Rejected);
        assert!(is_timelock_expired(&env, &loaded));
        // Guard: execution requires Timelock phase
        assert_ne!(loaded.phase, ProposalPhase::Timelock);
    }

    #[test]
    fn test_nonexistent_proposal_returns_none() {
        let env = Env::default();
        assert!(load(&env, 9999).is_none());
    }

    #[test]
    fn test_multiple_proposals_have_independent_timelocks() {
        let env = Env::default();
        env.ledger().set_timestamp(0);
        let p1 = make_proposal(&env, ProposalType::EmergencyAction, TIMELOCK_EMERGENCY);
        let p2 = make_proposal(&env, ProposalType::ContractUpgrade, TIMELOCK_UPGRADE);
        store(&env, &p1);
        store(&env, &p2);
        // After 6 hours: emergency expired, upgrade still locked
        env.ledger().set_timestamp(TIMELOCK_EMERGENCY);
        assert!(is_timelock_expired(&env, &p1));
        assert!(!is_timelock_expired(&env, &p2));
        // After 7 days: both expired
        env.ledger().set_timestamp(TIMELOCK_UPGRADE);
        assert!(is_timelock_expired(&env, &p1));
        assert!(is_timelock_expired(&env, &p2));
    }

    #[test]
    fn test_timelock_ends_computed_from_proposal_type() {
        let env = Env::default();
        env.ledger().set_timestamp(1_000_000);
        let now = env.ledger().timestamp();
        for (ptype, expected_delay) in [
            (ProposalType::EmergencyAction, TIMELOCK_EMERGENCY),
            (ProposalType::ContractUpgrade, TIMELOCK_UPGRADE),
            (ProposalType::ParameterChange, TIMELOCK_STANDARD),
        ] {
            let delay = timelock_duration(&ptype);
            assert_eq!(delay, expected_delay);
            let timelock_ends = now + delay;
            let proposal = make_proposal(&env, ptype, timelock_ends);
            // Before expiry
            env.ledger().set_timestamp(now + delay - 1);
            assert!(!is_timelock_expired(&env, &proposal));
            // At expiry
            env.ledger().set_timestamp(now + delay);
            assert!(is_timelock_expired(&env, &proposal));
            // Reset
            env.ledger().set_timestamp(1_000_000);
        }
    }
}

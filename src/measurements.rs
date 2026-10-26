//! Optional Yellowstone-only account measurements. Transaction processing stays upstream.
use crate::{
    analysis::{self, RunSummary},
    config::{Config, Endpoint, EndpointKind},
    proto::geyser::SubscribeUpdateAccount,
    utils::{Comparator, TransactionData},
};

struct Category {
    comparator: Comparator,
}
impl Category {
    fn new() -> Self {
        Self {
            comparator: Comparator::new(),
        }
    }
    fn record(&self, endpoint: &str, key: &str, data: TransactionData, producers: usize) {
        self.comparator
            .record_observation(endpoint, key, data, producers);
    }
    fn summary(&self, endpoints: &[String]) -> RunSummary {
        analysis::compute_run_summary(&self.comparator, endpoints)
    }
}
pub struct AdditionalMeasurements {
    endpoints: Vec<String>,
    accounts: Category,
}
impl AdditionalMeasurements {
    pub fn new(endpoints: &[Endpoint]) -> Self {
        Self {
            endpoints: endpoints
                .iter()
                .filter(|e| e.kind == EndpointKind::Yellowstone)
                .map(|e| e.name.clone())
                .collect(),
            accounts: Category::new(),
        }
    }
    pub fn record_account(
        &self,
        endpoint: &str,
        update: &SubscribeUpdateAccount,
        data: TransactionData,
    ) {
        if !self.endpoints.iter().any(|e| e == endpoint) || update.is_startup {
            return;
        }
        let Some(account) = &update.account else {
            return;
        };
        let Some(signature) = &account.txn_signature else {
            return;
        };
        if account.pubkey.len() != 32 || signature.len() != 64 {
            return;
        }
        // Write versions are validator-local. Compare the earliest notification
        // for the same slot, account and transaction across the gRPC endpoints.
        let key = format!(
            "{}:{}:{}",
            update.slot,
            bs58::encode(&account.pubkey).into_string(),
            bs58::encode(signature).into_string()
        );
        self.accounts
            .record(endpoint, &key, data, self.endpoints.len());
    }
    pub fn render(&self, config: &Config) -> String {
        if !config.yellowstone_accounts_bench_enabled {
            return String::new();
        }
        if self.endpoints.is_empty() {
            return "\nAccount statistics unavailable: no Yellowstone gRPC endpoints configured.\n"
                .into();
        }
        let mut output = String::new();
        output.push_str(&analysis::render_category_summary(
            &self.accounts.summary(&self.endpoints),
            "Account updates (Yellowstone only)",
            "Valid Acc",
        ));
        output.push_str(&format!(
            "Observed distinct account events: {}\n",
            self.accounts.comparator.len()
        ));
        output
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        proto::geyser::SubscribeUpdateAccountInfo,
        tests::{config, observation},
    };
    fn account(slot: u64, version: u64) -> SubscribeUpdateAccount {
        SubscribeUpdateAccount {
            slot,
            is_startup: false,
            account: Some(SubscribeUpdateAccountInfo {
                pubkey: vec![1; 32],
                txn_signature: Some(vec![2; 64]),
                write_version: version,
                ..Default::default()
            }),
        }
    }
    #[test]
    fn account_matching_ignores_local_versions_and_other_feed_kinds() {
        let mut c = config();
        c.config.yellowstone_accounts_bench_enabled = true;
        let m = AdditionalMeasurements::new(&c.endpoint);
        m.record_account("A", &account(1, 1), observation(1));
        m.record_account("B", &account(1, 999), observation(3));
        m.record_account("Other feed", &account(2, 1), observation(2));
        let summary = m.accounts.summary(&m.endpoints);
        assert_eq!(summary.total_signatures, 1);
        assert_eq!(summary.endpoints.len(), 2);
        assert_eq!(m.accounts.comparator.len(), 1);
        assert_eq!(
            summary
                .endpoints
                .iter()
                .find(|e| e.name == "B")
                .unwrap()
                .p50_delay_ms,
            Some(2.0)
        );
        let text = m.render(&c.config);
        assert!(text.contains("Valid Acc") && text.contains("Account updates"));
        assert!(!text.contains("Other feed"));
    }
    #[test]
    fn account_startup_unsigned_and_different_slots_do_not_match() {
        let c = config();
        let m = AdditionalMeasurements::new(&c.endpoint);
        let mut update = account(1, 1);
        update.is_startup = true;
        m.record_account("A", &update, observation(1));
        assert_eq!(m.accounts.comparator.len(), 0);
        update.is_startup = false;
        update.account.as_mut().unwrap().txn_signature = None;
        m.record_account("A", &update, observation(1));
        assert_eq!(m.accounts.comparator.len(), 0);
        m.record_account("A", &account(1, 1), observation(1));
        m.record_account("B", &account(2, 1), observation(2));
        assert_eq!(m.accounts.summary(&m.endpoints).total_signatures, 0);
    }
    #[test]
    fn multiple_account_events_are_retained_and_matched() {
        let c = config();
        let m = AdditionalMeasurements::new(&c.endpoint);
        m.record_account("A", &account(1, 1), observation(1));
        m.record_account("A", &account(2, 1), observation(2));
        m.record_account("B", &account(1, 2), observation(3));
        m.record_account("B", &account(2, 2), observation(4));
        assert_eq!(m.accounts.comparator.len(), 2);
        assert_eq!(m.accounts.summary(&m.endpoints).total_signatures, 2);
    }
    #[test]
    fn disabled_categories_render_nothing_and_non_grpc_cannot_participate() {
        let mut c = config();
        let m = AdditionalMeasurements::new(&c.endpoint);
        assert!(m.render(&c.config).is_empty());
        c.config.yellowstone_accounts_bench_enabled = true;
        c.endpoint.retain(|e| e.kind != EndpointKind::Yellowstone);
        let m = AdditionalMeasurements::new(&c.endpoint);
        assert!(m.render(&c.config).contains("unavailable"));
    }
}

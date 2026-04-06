use crate::runtime::state_store::StateStore;
use crate::runtime::types::{AuthorityLease, EventEnvelope, EventKind, RunRecord, SCHEMA_VERSION};
use chrono::{Duration, Utc};
use serde_json::Map;

pub fn renew_authority(
    store: &StateStore,
    run_id: &str,
    owner: &str,
    lease_minutes: i64,
) -> Result<RunRecord, String> {
    let mut run = store.read_run(run_id)?;
    let now = Utc::now();
    let lease_id = run
        .authority
        .as_ref()
        .map(|lease| lease.lease_id.clone())
        .unwrap_or_else(|| format!("lease-{run_id}"));

    run.authority = Some(AuthorityLease {
        owner: owner.to_string(),
        lease_id,
        leased_until: now + Duration::minutes(lease_minutes.max(1)),
        stale: false,
    });
    run.updated_at = now;
    store.write_run(&run)?;
    store.append_runtime_event(
        run_id,
        EventEnvelope {
            schema_version: SCHEMA_VERSION,
            event: EventKind::AuthorityRenewed,
            timestamp: now,
            run_id: Some(run_id.to_string()),
            session_id: None,
            source: "authority".to_string(),
            worker: Some(owner.to_string()),
            task_id: None,
            message_id: None,
            reason: None,
            context: Map::new(),
        },
    )?;
    store.refresh_snapshot(run_id)?;
    Ok(run)
}

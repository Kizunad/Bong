use std::io;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::cultivation::life_record::LifeRecord;
use crate::persistence::{open_persistence_connection, PersistenceSettings};

use super::components::{VoidActionKind, VoidActionLogEntry};

pub const LEGACY_REJECTION_WINDOW_TICKS: u64 = 24 * 60 * 60 * 20;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LegacyLetterboxStatus {
    Pending,
    Accepted,
    Rejected,
    Drifted,
    Finalized,
}

impl Default for LegacyLetterboxStatus {
    fn default() -> Self {
        Self::Pending
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LegacyLetterbox {
    pub owner_id: String,
    pub inheritor_id: String,
    pub item_instance_ids: Vec<u64>,
    #[serde(default)]
    pub message: Option<String>,
    pub assigned_at_tick: u64,
    pub reject_until_tick: u64,
    #[serde(default)]
    pub status: LegacyLetterboxStatus,
    #[serde(default)]
    pub finalized_at_tick: Option<u64>,
}

impl LegacyLetterbox {
    pub fn new(
        owner_id: impl Into<String>,
        inheritor_id: impl Into<String>,
        item_instance_ids: Vec<u64>,
        message: Option<String>,
        assigned_at_tick: u64,
    ) -> Self {
        Self {
            owner_id: owner_id.into(),
            inheritor_id: inheritor_id.into(),
            item_instance_ids,
            message,
            assigned_at_tick,
            reject_until_tick: assigned_at_tick.saturating_add(LEGACY_REJECTION_WINDOW_TICKS),
            status: LegacyLetterboxStatus::Pending,
            finalized_at_tick: None,
        }
    }

    pub fn can_reject_at(&self, now_tick: u64) -> bool {
        self.status == LegacyLetterboxStatus::Pending && now_tick <= self.reject_until_tick
    }

    pub fn mark_rejected(&mut self, now_tick: u64) -> bool {
        if !self.can_reject_at(now_tick) {
            return false;
        }
        self.status = LegacyLetterboxStatus::Rejected;
        true
    }

    pub fn finalize(&mut self, now_tick: u64) {
        self.status = LegacyLetterboxStatus::Finalized;
        self.finalized_at_tick = Some(now_tick);
    }

    pub fn drift_if_expired(&mut self, now_tick: u64) -> bool {
        if self.status == LegacyLetterboxStatus::Pending && now_tick > self.reject_until_tick {
            self.status = LegacyLetterboxStatus::Drifted;
            return true;
        }
        false
    }
}

pub fn assign_legacy(
    life_record: &mut LifeRecord,
    owner_id: &str,
    inheritor_id: &str,
    item_instance_ids: Vec<u64>,
    message: Option<String>,
    now_tick: u64,
) -> LegacyLetterbox {
    let letterbox = LegacyLetterbox::new(
        owner_id.to_string(),
        inheritor_id.to_string(),
        item_instance_ids.clone(),
        message,
        now_tick,
    );
    apply_legacy_assignment(life_record, letterbox.clone());
    letterbox
}

pub fn apply_legacy_assignment(life_record: &mut LifeRecord, letterbox: LegacyLetterbox) {
    life_record.legacy_inheritor = Some(letterbox.inheritor_id.clone());
    life_record.legacy_items = letterbox.item_instance_ids.clone();
    life_record.legacy_letterbox = Some(letterbox.clone());
    life_record.void_actions.push(VoidActionLogEntry::accepted(
        VoidActionKind::LegacyAssign,
        &letterbox.inheritor_id,
        letterbox.assigned_at_tick,
        "legacy_assigned",
    ));
}

pub fn persist_legacy_letterbox(
    settings: &PersistenceSettings,
    letterbox: &LegacyLetterbox,
) -> io::Result<()> {
    let connection = open_persistence_connection(settings)?;
    let payload_json = serde_json::to_string(letterbox)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    connection
        .execute(
            "
            INSERT INTO legacy_letterbox (
                owner_id,
                inheritor_id,
                payload_json,
                assigned_at_tick,
                reject_until_tick,
                status,
                schema_version,
                last_updated_wall
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, unixepoch())
            ON CONFLICT(owner_id) DO UPDATE SET
                inheritor_id = excluded.inheritor_id,
                payload_json = excluded.payload_json,
                assigned_at_tick = excluded.assigned_at_tick,
                reject_until_tick = excluded.reject_until_tick,
                status = excluded.status,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                letterbox.owner_id,
                letterbox.inheritor_id,
                payload_json,
                letterbox.assigned_at_tick,
                letterbox.reject_until_tick,
                format!("{:?}", letterbox.status).to_ascii_lowercase(),
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub fn load_legacy_letterbox(
    settings: &PersistenceSettings,
    owner_id: &str,
) -> io::Result<Option<LegacyLetterbox>> {
    let connection = open_persistence_connection(settings)?;
    let payload_json: Option<String> = connection
        .query_row(
            "SELECT payload_json FROM legacy_letterbox WHERE owner_id = ?1",
            params![owner_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;
    payload_json
        .map(|payload| {
            serde_json::from_str::<LegacyLetterbox>(&payload)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_letterbox_defaults_to_pending() {
        let letterbox = LegacyLetterbox::new("owner", "heir", vec![1], None, 10);
        assert_eq!(letterbox.status, LegacyLetterboxStatus::Pending);
    }

    #[test]
    fn legacy_letterbox_has_24h_reject_window() {
        let letterbox = LegacyLetterbox::new("owner", "heir", vec![1], None, 10);
        assert_eq!(
            letterbox.reject_until_tick,
            10 + LEGACY_REJECTION_WINDOW_TICKS
        );
    }

    #[test]
    fn legacy_letterbox_can_reject_before_deadline() {
        let letterbox = LegacyLetterbox::new("owner", "heir", vec![1], None, 10);
        assert!(letterbox.can_reject_at(10 + LEGACY_REJECTION_WINDOW_TICKS));
    }

    #[test]
    fn legacy_letterbox_cannot_reject_after_deadline() {
        let letterbox = LegacyLetterbox::new("owner", "heir", vec![1], None, 10);
        assert!(!letterbox.can_reject_at(11 + LEGACY_REJECTION_WINDOW_TICKS));
    }

    #[test]
    fn mark_rejected_updates_status() {
        let mut letterbox = LegacyLetterbox::new("owner", "heir", vec![1], None, 10);
        assert!(letterbox.mark_rejected(20));
        assert_eq!(letterbox.status, LegacyLetterboxStatus::Rejected);
    }

    #[test]
    fn mark_rejected_after_deadline_fails() {
        let mut letterbox = LegacyLetterbox::new("owner", "heir", vec![1], None, 10);
        assert!(!letterbox.mark_rejected(11 + LEGACY_REJECTION_WINDOW_TICKS));
        assert_eq!(letterbox.status, LegacyLetterboxStatus::Pending);
    }

    #[test]
    fn finalize_sets_tick() {
        let mut letterbox = LegacyLetterbox::new("owner", "heir", vec![1], None, 10);
        letterbox.finalize(99);
        assert_eq!(letterbox.status, LegacyLetterboxStatus::Finalized);
        assert_eq!(letterbox.finalized_at_tick, Some(99));
    }

    #[test]
    fn drift_if_expired_marks_drifted() {
        let mut letterbox = LegacyLetterbox::new("owner", "heir", vec![1], None, 10);
        assert!(letterbox.drift_if_expired(11 + LEGACY_REJECTION_WINDOW_TICKS));
        assert_eq!(letterbox.status, LegacyLetterboxStatus::Drifted);
    }

    #[test]
    fn drift_if_expired_keeps_pending_before_deadline() {
        let mut letterbox = LegacyLetterbox::new("owner", "heir", vec![1], None, 10);
        assert!(!letterbox.drift_if_expired(10 + LEGACY_REJECTION_WINDOW_TICKS));
        assert_eq!(letterbox.status, LegacyLetterboxStatus::Pending);
    }

    #[test]
    fn assign_legacy_updates_life_record_heir() {
        let mut life = LifeRecord::new("owner");
        assign_legacy(&mut life, "owner", "heir", vec![1], None, 10);
        assert_eq!(life.legacy_inheritor.as_deref(), Some("heir"));
    }

    #[test]
    fn assign_legacy_updates_life_record_items() {
        let mut life = LifeRecord::new("owner");
        assign_legacy(&mut life, "owner", "heir", vec![1, 2], None, 10);
        assert_eq!(life.legacy_items, vec![1, 2]);
    }

    #[test]
    fn assign_legacy_stores_letterbox() {
        let mut life = LifeRecord::new("owner");
        assign_legacy(
            &mut life,
            "owner",
            "heir",
            vec![1],
            Some("take it".to_string()),
            10,
        );
        assert_eq!(
            life.legacy_letterbox
                .as_ref()
                .and_then(|l| l.message.as_deref()),
            Some("take it")
        );
    }

    #[test]
    fn assign_legacy_records_void_action() {
        let mut life = LifeRecord::new("owner");
        assign_legacy(&mut life, "owner", "heir", vec![1], None, 10);
        assert_eq!(life.void_actions.len(), 1);
        assert_eq!(life.void_actions[0].kind, VoidActionKind::LegacyAssign);
    }

    #[test]
    fn legacy_letterbox_serializes_status_snake_case() {
        let letterbox = LegacyLetterbox::new("owner", "heir", vec![1], None, 10);
        let value = serde_json::to_value(letterbox).expect("letterbox should serialize");
        assert_eq!(value["status"], "pending");
    }
}

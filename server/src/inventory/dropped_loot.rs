//! 掉落物 metadata 与旧 JSON 的纯迁移 seam。
//!
//! 这里不负责 world broadcast、拾取授权或 registry writer；这些行为仍由各自 owner
//! 在后续 R3/R6/R10 merge unit 中接入。模块只保证旧 entry 缺少 owner/visibility 时
//! 显式得到公共可见的默认值，并对未来 schema fail closed。

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 掉落物可见性契约；它是 recipient-specific projection 的输入，而不是客户端权限。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DroppedLootVisibility {
    /// 同维且在授权观察范围内的普通掉落。
    Public,
    /// 只向 owner 或 server-authorized administrator 投影的私人掉落。
    OwnerOnly,
}

/// 掉落物 owner/visibility 的纯 metadata 投影。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DroppedLootMetadata {
    /// 私人掉落的 canonical player id；公共掉落必须为 None。
    pub owner: Option<String>,
    /// recipient projection 使用的可见性。
    pub visibility: DroppedLootVisibility,
}

impl Default for DroppedLootMetadata {
    fn default() -> Self {
        Self {
            owner: None,
            visibility: DroppedLootVisibility::Public,
        }
    }
}

/// 掉落物迁移失败的原因；失败时调用方应保留旧 entry_json。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DroppedLootMigrationError {
    /// JSON 根值不是 object。
    RootNotObject,
    /// persisted schema 高于当前代码。
    UnsupportedSchema { found: i32, supported: i32 },
    /// owner 只能是 null 或 canonical player id 字符串。
    InvalidOwner,
    /// visibility 只能是 `public` 或 `owner_only`。
    InvalidVisibility,
    /// `public` 必须没有 owner，`owner_only` 必须有 owner。
    InvalidMetadataCombination {
        visibility: DroppedLootVisibility,
        owner_present: bool,
    },
}

/// 掉落物 metadata 当前能理解的最高 schema 版本。
pub const CURRENT_DROPPED_LOOT_SCHEMA_VERSION: i32 = 1;

/// 对一条旧 dropped-loot JSON 做纯、幂等迁移。
///
/// 两个字段都缺失的历史行明确补成 `None`/`Public`；已有字段和值保持原样，并要求
/// 现存字段组成合法的 owner/visibility 对。函数不执行 SQL、不分配实体、不写 registry，
/// 因此失败时可以安全重试。
pub fn migrate_legacy_dropped_loot_entry(
    value: Value,
    schema_version: i32,
) -> Result<Value, DroppedLootMigrationError> {
    if schema_version > CURRENT_DROPPED_LOOT_SCHEMA_VERSION {
        return Err(DroppedLootMigrationError::UnsupportedSchema {
            found: schema_version,
            supported: CURRENT_DROPPED_LOOT_SCHEMA_VERSION,
        });
    }
    let mut root = value
        .as_object()
        .cloned()
        .ok_or(DroppedLootMigrationError::RootNotObject)?;

    validate_owner(root.get("owner"))?;
    validate_visibility(root.get("visibility"))?;
    let owner = root.get("owner").and_then(Value::as_str);
    let visibility = match root.get("visibility").and_then(Value::as_str) {
        None | Some("public") => DroppedLootVisibility::Public,
        Some("owner_only") => DroppedLootVisibility::OwnerOnly,
        Some(_) => unreachable!("validate_visibility checked persisted visibility"),
    };
    validate_metadata_combination(owner, visibility)?;
    root.entry("owner".to_string()).or_insert(Value::Null);
    root.entry("visibility".to_string())
        .or_insert(Value::String("public".to_string()));
    Ok(Value::Object(root))
}

/// 将 metadata 编码为 entry JSON 字段，供后续 hydration/provider 复用。
pub fn apply_dropped_loot_metadata(value: &mut Value, metadata: &DroppedLootMetadata) -> bool {
    if validate_metadata_combination(metadata.owner.as_deref(), metadata.visibility).is_err() {
        return false;
    }
    let Some(root) = value.as_object_mut() else {
        return false;
    };
    root.insert(
        "owner".to_string(),
        metadata
            .owner
            .as_ref()
            .map_or(Value::Null, |owner| Value::String(owner.clone())),
    );
    root.insert(
        "visibility".to_string(),
        Value::String(
            match metadata.visibility {
                DroppedLootVisibility::Public => "public",
                DroppedLootVisibility::OwnerOnly => "owner_only",
            }
            .to_string(),
        ),
    );
    true
}

fn validate_owner(value: Option<&Value>) -> Result<(), DroppedLootMigrationError> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.is_null() || value.as_str().is_some_and(is_canonical_owner_id) {
        Ok(())
    } else {
        Err(DroppedLootMigrationError::InvalidOwner)
    }
}

fn validate_visibility(value: Option<&Value>) -> Result<(), DroppedLootMigrationError> {
    let Some(value) = value else {
        return Ok(());
    };
    match value.as_str() {
        Some("public") | Some("owner_only") => Ok(()),
        _ => Err(DroppedLootMigrationError::InvalidVisibility),
    }
}

fn validate_metadata_combination(
    owner: Option<&str>,
    visibility: DroppedLootVisibility,
) -> Result<(), DroppedLootMigrationError> {
    if owner.is_some_and(|owner| !is_canonical_owner_id(owner)) {
        return Err(DroppedLootMigrationError::InvalidOwner);
    }
    let owner_present = owner.is_some();
    let valid = match visibility {
        DroppedLootVisibility::Public => !owner_present,
        DroppedLootVisibility::OwnerOnly => owner_present,
    };
    if valid {
        Ok(())
    } else {
        Err(DroppedLootMigrationError::InvalidMetadataCombination {
            visibility,
            owner_present,
        })
    }
}

/// 当前身份层的 canonical id 至少必须是一个无空白的非空 token；具体前缀由身份 owner
/// 生成（例如 `offline:<username>`），本层不猜测跨域身份格式。
fn is_canonical_owner_id(owner: &str) -> bool {
    !owner.is_empty() && owner == owner.trim() && !owner.chars().any(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn legacy_entry_defaults_missing_metadata_without_touching_existing_fields() {
        let value = json!({"instance_id": 7, "item": {"template_id": "ore"}});
        let migrated = migrate_legacy_dropped_loot_entry(value, 0).expect("migration succeeds");
        assert_eq!(migrated["instance_id"], 7);
        assert_eq!(migrated["owner"], Value::Null);
        assert_eq!(migrated["visibility"], "public");
        let again = migrate_legacy_dropped_loot_entry(migrated.clone(), 1).expect("idempotent");
        assert_eq!(again, migrated);
    }

    #[test]
    fn existing_owner_only_metadata_is_preserved() {
        let value = json!({"owner": "char:alice", "visibility": "owner_only"});
        let migrated = migrate_legacy_dropped_loot_entry(value.clone(), 1).expect("valid");
        assert_eq!(migrated, value);
    }

    #[test]
    fn malformed_metadata_fails_closed() {
        let invalid_owner = json!({"owner": 42});
        assert_eq!(
            migrate_legacy_dropped_loot_entry(invalid_owner, 1),
            Err(DroppedLootMigrationError::InvalidOwner)
        );
        let blank_owner = json!({"owner": "   ", "visibility": "owner_only"});
        assert_eq!(
            migrate_legacy_dropped_loot_entry(blank_owner, 1),
            Err(DroppedLootMigrationError::InvalidOwner)
        );
        let invalid_visibility = json!({"visibility": "private"});
        assert_eq!(
            migrate_legacy_dropped_loot_entry(invalid_visibility, 1),
            Err(DroppedLootMigrationError::InvalidVisibility)
        );
    }

    #[test]
    fn metadata_migration_rejects_inconsistent_owner_visibility_pairs() {
        for (value, expected) in [
            (
                json!({"owner": "char:alice", "visibility": "public"}),
                DroppedLootVisibility::Public,
            ),
            (
                json!({"owner": null, "visibility": "owner_only"}),
                DroppedLootVisibility::OwnerOnly,
            ),
        ] {
            assert_eq!(
                migrate_legacy_dropped_loot_entry(value, 1),
                Err(DroppedLootMigrationError::InvalidMetadataCombination {
                    visibility: expected,
                    owner_present: expected == DroppedLootVisibility::Public,
                })
            );
        }
    }

    #[test]
    fn applying_invalid_metadata_does_not_mutate_the_entry() {
        let mut value = json!({"instance_id": 7, "owner": null, "visibility": "public"});
        let before = value.clone();
        assert!(!apply_dropped_loot_metadata(
            &mut value,
            &DroppedLootMetadata {
                owner: Some("char:alice".to_string()),
                visibility: DroppedLootVisibility::Public,
            }
        ));
        assert_eq!(value, before);

        assert!(!apply_dropped_loot_metadata(
            &mut value,
            &DroppedLootMetadata {
                owner: None,
                visibility: DroppedLootVisibility::OwnerOnly,
            }
        ));
        assert_eq!(value, before);

        assert!(!apply_dropped_loot_metadata(
            &mut value,
            &DroppedLootMetadata {
                owner: Some("   ".to_string()),
                visibility: DroppedLootVisibility::OwnerOnly,
            }
        ));
        assert_eq!(value, before);
    }
}

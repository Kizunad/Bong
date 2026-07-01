use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, App, Resource};

/// F25 加固 — `field_mut()` 的 match 臂与 `FIELD_NAMES` 数组此前是两份平行手写
/// 列表，仅靠运行期测试互校，新增/删字段漏改一处编译期无保护。
///
/// 用单一宏调用同时生成 struct 字段 / `Default` / `field_mut` 的 match 分支 /
/// `FIELD_NAMES` 数组，遗漏字段会直接变成"字段名列表少写一个"这一处编译错误，
/// 而不是四处手写列表里悄悄漏掉一处。公开 API（字段名 / `field_mut` 签名 /
/// `FIELD_NAMES`）保持不变。
macro_rules! shader_state_fields {
    ($($field:ident),+ $(,)?) => {
        #[derive(Debug, Clone, Serialize, Deserialize, Resource)]
        pub struct ShaderStatePayload {
            $(pub $field: f32,)+
        }

        impl Default for ShaderStatePayload {
            fn default() -> Self {
                Self {
                    $($field: 0.0,)+
                }
            }
        }

        impl ShaderStatePayload {
            pub fn field_mut(&mut self, name: &str) -> Option<&mut f32> {
                match name {
                    $(stringify!($field) => Some(&mut self.$field),)+
                    _ => None,
                }
            }

            pub const FIELD_NAMES: &'static [&'static str] = &[
                $(stringify!($field)),+
            ];
        }
    };
}

shader_state_fields!(
    bong_realm,
    bong_lingqi,
    bong_tribulation,
    bong_enlightenment,
    bong_inkwash,
    bong_bloodmoon,
    bong_meditation,
    bong_demonic,
    bong_wind_strength,
    bong_wind_angle,
);

impl ShaderStatePayload {
    pub fn to_json_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("ShaderStatePayload serialization should never fail")
    }
}

pub fn register(app: &mut App) {
    app.insert_resource(ShaderStatePayload::default());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_all_zeros() {
        let payload = ShaderStatePayload::default();
        assert_eq!(payload.bong_realm, 0.0);
        assert_eq!(payload.bong_lingqi, 0.0);
        assert_eq!(payload.bong_tribulation, 0.0);
        assert_eq!(payload.bong_enlightenment, 0.0);
        assert_eq!(payload.bong_inkwash, 0.0);
        assert_eq!(payload.bong_bloodmoon, 0.0);
        assert_eq!(payload.bong_meditation, 0.0);
        assert_eq!(payload.bong_demonic, 0.0);
        assert_eq!(payload.bong_wind_strength, 0.0);
        assert_eq!(payload.bong_wind_angle, 0.0);
    }

    #[test]
    fn serializes_to_valid_json() {
        let payload = ShaderStatePayload {
            bong_bloodmoon: 0.8,
            bong_wind_angle: 3.12,
            ..Default::default()
        };
        let bytes = payload.to_json_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("should be valid JSON");
        assert_eq!(json["bong_bloodmoon"], 0.8f64);
        assert!((json["bong_wind_angle"].as_f64().unwrap() - 3.12).abs() < 0.001);
    }

    #[test]
    fn field_mut_all_known() {
        let mut payload = ShaderStatePayload::default();
        for name in ShaderStatePayload::FIELD_NAMES {
            let field = payload.field_mut(name);
            assert!(
                field.is_some(),
                "field_mut should return Some for known field: {name}"
            );
        }
    }

    #[test]
    fn field_mut_unknown_returns_none() {
        let mut payload = ShaderStatePayload::default();
        assert!(payload.field_mut("bong_nonexistent").is_none());
        assert!(payload.field_mut("").is_none());
        assert!(payload.field_mut("realm").is_none());
    }

    #[test]
    fn field_names_count_matches_struct() {
        assert_eq!(
            ShaderStatePayload::FIELD_NAMES.len(),
            10,
            "Expected 10 fields matching 10 BongUniform variants"
        );
    }

    #[test]
    fn field_mut_write_read_round_trip() {
        let mut payload = ShaderStatePayload::default();
        *payload.field_mut("bong_bloodmoon").unwrap() = 0.75;
        assert_eq!(payload.bong_bloodmoon, 0.75);
    }

    #[test]
    fn deserializes_from_json() {
        let json = r#"{"bong_realm":0.5,"bong_lingqi":0.3,"bong_tribulation":0.0,"bong_enlightenment":0.0,"bong_inkwash":0.0,"bong_bloodmoon":1.0,"bong_meditation":0.0,"bong_demonic":0.0,"bong_wind_strength":0.7,"bong_wind_angle":1.57}"#;
        let payload: ShaderStatePayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.bong_realm, 0.5);
        assert_eq!(payload.bong_bloodmoon, 1.0);
        assert_eq!(payload.bong_wind_strength, 0.7);
        assert!((payload.bong_wind_angle - 1.57).abs() < 0.001);
    }
}

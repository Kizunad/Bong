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

use serde::{Deserialize, Serialize};

/// TSY 起源分类。运行时仍从 family/zone name 前缀推导，避免扩展 `Zone` 持久结构。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TsyOrigin {
    DanengLuoluo,
    ZongmenYiji,
    ZhanchangChendian,
    GaoshouShichu,
}

impl TsyOrigin {
    pub fn from_zone_name(name: &str) -> Option<Self> {
        let body = name.strip_prefix("tsy_")?;
        if body.starts_with("daneng_") || body.starts_with("tankuozun") {
            Some(Self::DanengLuoluo)
        } else if body.starts_with("zongmen_") || body.starts_with("lingxu_") {
            Some(Self::ZongmenYiji)
        } else if body.starts_with("zhanchang_") {
            Some(Self::ZhanchangChendian)
        } else if body.starts_with("gaoshou_") {
            Some(Self::GaoshouShichu)
        } else {
            None
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "daneng_luoluo" => Some(Self::DanengLuoluo),
            "zongmen_yiji" => Some(Self::ZongmenYiji),
            "zhanchang_chendian" => Some(Self::ZhanchangChendian),
            "gaoshou_shichu" => Some(Self::GaoshouShichu),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_origin_from_family_or_layer_name() {
        assert_eq!(
            TsyOrigin::from_zone_name("tsy_daneng_01_deep"),
            Some(TsyOrigin::DanengLuoluo)
        );
        assert_eq!(
            TsyOrigin::from_zone_name("tsy_zongmen_lingxu_01_deep"),
            Some(TsyOrigin::ZongmenYiji)
        );
        assert_eq!(
            TsyOrigin::from_zone_name("tsy_lingxu_01"),
            Some(TsyOrigin::ZongmenYiji)
        );
        assert_eq!(
            TsyOrigin::from_zone_name("tsy_zhanchang_old_mid"),
            Some(TsyOrigin::ZhanchangChendian)
        );
        assert_eq!(
            TsyOrigin::from_zone_name("tsy_gaoshou_shichu_02"),
            Some(TsyOrigin::GaoshouShichu)
        );
        assert_eq!(TsyOrigin::from_zone_name("spawn"), None);
    }
}

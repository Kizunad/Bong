use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

use super::common::{NarrationKind, NarrationScope, NarrationStyle, MAX_NARRATION_LENGTH};

#[derive(Debug, Clone, Serialize)]
pub struct Narration {
    pub scope: NarrationScope,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    pub text: String,
    pub style: NarrationStyle,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<NarrationKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NarrationV1 {
    #[serde(deserialize_with = "deserialize_v1_version")]
    pub v: u8,
    pub narrations: Vec<Narration>,
}

impl<'de> Deserialize<'de> for Narration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct NarrationWire {
            scope: NarrationScope,
            #[serde(default)]
            target: Option<String>,
            text: String,
            style: NarrationStyle,
            #[serde(default)]
            kind: Option<NarrationKind>,
        }

        let wire = NarrationWire::deserialize(deserializer)?;
        if wire.scope != NarrationScope::Broadcast && wire.target.is_none() {
            return Err(D::Error::custom(format!(
                "Narration.target is required when scope is `{:?}`",
                wire.scope
            )));
        }

        if wire.text.chars().count() > MAX_NARRATION_LENGTH {
            return Err(D::Error::custom(format!(
                "Narration.text exceeds {MAX_NARRATION_LENGTH} characters"
            )));
        }

        Ok(Self {
            scope: wire.scope,
            target: wire.target,
            text: wire.text,
            style: wire.style,
            kind: wire.kind,
        })
    }
}

fn deserialize_v1_version<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let version = u8::deserialize(deserializer)?;
    if version == 1 {
        Ok(version)
    } else {
        Err(D::Error::custom(format!(
            "NarrationV1.v must be 1, got {version}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::channels::CH_AGENT_NARRATE;
    use serde_json::{json, Value};

    fn sample_narration_value() -> Value {
        serde_json::from_str(include_str!(
            "../../../agent/packages/schema/samples/narration.sample.json"
        ))
        .expect("narration sample should parse into JSON value")
    }

    #[test]
    fn deserialize_narration_sample() {
        let json = include_str!("../../../agent/packages/schema/samples/narration.sample.json");
        let msg: NarrationV1 = serde_json::from_str(json)
            .expect("narration.sample.json should deserialize into NarrationV1");

        assert_eq!(msg.v, 1);
        assert_eq!(msg.narrations.len(), 2);
        assert_eq!(msg.narrations[0].scope, NarrationScope::Broadcast);
        assert_eq!(msg.narrations[0].style, NarrationStyle::SystemWarning);
        assert!(msg.narrations[0].text.contains("天道震怒"));
        assert_eq!(msg.narrations[1].scope, NarrationScope::Player);
        assert_eq!(msg.narrations[1].target.as_deref(), Some("offline:Steve"));
        assert_eq!(CH_AGENT_NARRATE, "bong:agent_narrate");
    }

    #[test]
    fn deserialize_narration_sample_rejects_wrong_version() {
        let mut value = sample_narration_value();
        value["v"] = json!(2);

        assert!(serde_json::from_value::<NarrationV1>(value).is_err());
    }

    #[test]
    fn deserialize_narration_sample_rejects_unknown_top_level_field() {
        let mut value = sample_narration_value();
        value["trace_id"] = json!("narration-1");

        assert!(serde_json::from_value::<NarrationV1>(value).is_err());
    }

    #[test]
    fn deserialize_narration_sample_rejects_unknown_nested_field() {
        let mut value = sample_narration_value();
        value["narrations"][0]["audience"] = json!("sect_leaders");

        assert!(serde_json::from_value::<NarrationV1>(value).is_err());
    }

    #[test]
    fn deserialize_narration_sample_rejects_missing_target_for_scoped_entry() {
        let mut value = sample_narration_value();
        let narration = value["narrations"][1]
            .as_object_mut()
            .expect("player-scoped narration should be an object");
        narration.remove("target");

        assert!(serde_json::from_value::<NarrationV1>(value).is_err());
    }

    #[test]
    fn deserialize_narration_sample_rejects_invalid_style() {
        let mut value = sample_narration_value();
        value["narrations"][0]["style"] = json!("ominous_whisper");

        assert!(serde_json::from_value::<NarrationV1>(value).is_err());
    }
}

use serde::{Deserialize, Serialize};

use super::common::{NarrationScope, NarrationStyle};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Narration {
    pub scope: NarrationScope,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    pub text: String,
    pub style: NarrationStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrationV1 {
    pub v: u8,
    pub narrations: Vec<Narration>,
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}

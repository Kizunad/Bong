use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageV1 {
    pub v: u8,
    pub ts: u64,
    pub player: String,
    pub raw: String,
    pub zone: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_chat_message_sample() {
        let json = include_str!("../../../agent/packages/schema/samples/chat-message.sample.json");
        let msg: ChatMessageV1 = serde_json::from_str(json)
            .expect("chat-message.sample.json should deserialize into ChatMessageV1");

        assert_eq!(msg.v, 1);
        assert_eq!(msg.player, "offline:Steve");
        assert!(msg.raw.contains("灵气"));
        assert_eq!(msg.zone, "blood_valley");
    }
}

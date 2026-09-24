use super::scroll::{try_into_scroll_request, ScrollRequest};
use crate::schema::client_request::ClientRequestV1;

#[test]
fn scroll_extractor_preserves_each_learning_variant_instance_id() {
    let cases = [
        (
            ClientRequestV1::LearnSkillScroll {
                v: 1,
                instance_id: u64::MAX,
            },
            ScrollRequest::LearnSkillScroll {
                instance_id: u64::MAX,
            },
        ),
        (
            ClientRequestV1::TechniqueScrollUse {
                v: 1,
                instance_id: u64::MAX - 1,
            },
            ScrollRequest::TechniqueScrollUse {
                instance_id: u64::MAX - 1,
            },
        ),
    ];

    for (wire_request, expected) in cases {
        assert_eq!(
            try_into_scroll_request(wire_request).ok(),
            Some(expected),
            "the typed scroll route must preserve the instance_id for each learning variant"
        );
    }
}

#[test]
fn non_scroll_request_is_returned_unchanged_to_the_next_route() {
    let request = ClientRequestV1::BreakthroughRequest { v: 1 };
    assert!(
        matches!(
            try_into_scroll_request(request),
            Err(ClientRequestV1::BreakthroughRequest { v: 1 })
        ),
        "a non-scroll request must remain available to the next typed or legacy route"
    );
}

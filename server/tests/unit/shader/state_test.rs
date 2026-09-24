use bong_server::shader::ShaderStatePayload;

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

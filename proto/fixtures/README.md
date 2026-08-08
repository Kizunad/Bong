# Shared production protobuf fixtures

`alchemy_session_active_v1.pb` and `alchemy_session_finished_v1.pb` are generated only by the
Rust server test helper. The generator calls the real `build_session_data` builder and the
production `ServerDataEnvelope` protobuf encoder; Fabric tests consume those exact bytes.

Regenerate deliberately from the repository root:

```bash
scripts/build-token.sh cargo test network::alchemy_snapshot_emit::tests::regenerate_alchemy_session_production_proto_fixtures -- --ignored --exact --nocapture
```

Ordinary Rust tests never modify these files. They regenerate the expected bytes in memory and
compare them byte-for-byte with the checked-in fixtures, so a production builder or wire change
fails until the shared fixtures are consciously reviewed and regenerated.

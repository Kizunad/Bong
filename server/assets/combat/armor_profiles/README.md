Armor profile blueprints live in this directory as one JSON file per item template.

Runtime code loads `server/assets/combat/armor_profiles/*.json` during combat startup and indexes profiles by `template_id`. Do not add a parallel Rust hard-coded armor table; tests assert this asset-backed convention.

Each file uses this shape:

```json
{
  "template_id": "fake_spirit_hide",
  "profile": {
    "slot": "chest",
    "body_coverage": ["chest", "abdomen"],
    "kind_mitigation": {
      "cut": 0.25,
      "blunt": 0.30,
      "pierce": 0.20,
      "burn": 0.10,
      "concussion": 0.15
    },
    "durability_max": 100,
    "broken_multiplier": 0.3
  }
}
```

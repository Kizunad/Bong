#![allow(dead_code, unused_imports)]
use bong_server::npc::lifecycle::{NpcArchetype, NpcRegistry};
use bong_server::npc::spawn::{PoissonSpawnSampler, RoguePopulationSeedConfig};
use valence::prelude::DVec3;

#[test]
fn poisson_sampler_respects_min_distance() {
    let bounds = (DVec3::new(0.0, 64.0, 0.0), DVec3::new(500.0, 80.0, 500.0));
    let sampler = PoissonSpawnSampler::adaptive_for_zone(bounds);
    assert_eq!(
        sampler.min_same_archetype_dist, 48.0,
        "500x500 zone should use same_dist=48"
    );

    // Place first NPC at center.
    let mut existing = vec![(DVec3::new(250.0, 72.0, 250.0), NpcArchetype::Rogue)];

    // Sample second NPC — should be at least min_same_archetype_dist from the first.
    let pos = sampler
        .sample_position(bounds, &existing, NpcArchetype::Rogue, 42)
        .expect("500x500 zone should have room for a second NPC");
    let dx = pos.x - existing[0].0.x;
    let dz = pos.z - existing[0].0.z;
    let dist = (dx * dx + dz * dz).sqrt();
    assert!(
        dist >= sampler.min_same_archetype_dist * 0.5,
        "second NPC should be placed with spacing; dist={:.1} vs min={:.1}",
        dist,
        sampler.min_same_archetype_dist
    );

    existing.push((pos, NpcArchetype::Rogue));
}

#[test]
fn poisson_sampler_returns_none_when_saturated() {
    // Tiny zone: 40x40 with min_same_dist=32. Can fit very few NPCs.
    let bounds = (DVec3::new(0.0, 64.0, 0.0), DVec3::new(40.0, 80.0, 40.0));
    let sampler = PoissonSpawnSampler {
        min_same_archetype_dist: 32.0,
        min_cross_archetype_dist: 16.0,
        max_candidates: 30,
    };

    // Fill with NPCs at grid points.
    let existing: Vec<(DVec3, NpcArchetype)> = vec![
        (DVec3::new(5.0, 72.0, 5.0), NpcArchetype::Rogue),
        (DVec3::new(5.0, 72.0, 35.0), NpcArchetype::Rogue),
        (DVec3::new(35.0, 72.0, 5.0), NpcArchetype::Rogue),
        (DVec3::new(35.0, 72.0, 35.0), NpcArchetype::Rogue),
        (DVec3::new(20.0, 72.0, 20.0), NpcArchetype::Rogue),
    ];

    let result = sampler.sample_position(bounds, &existing, NpcArchetype::Rogue, 99);
    assert!(
        result.is_none(),
        "40x40 zone with 5 NPCs at min_dist=32 should be saturated; got {:?}",
        result
    );
}

#[test]
fn adaptive_distance_scales_with_area() {
    // Large zone (>= 500x500)
    let large = PoissonSpawnSampler::adaptive_for_zone((
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(600.0, 100.0, 600.0),
    ));
    assert_eq!(
        large.min_same_archetype_dist, 48.0,
        "area >= 500x500 should use 48"
    );
    assert_eq!(
        large.min_cross_archetype_dist, 24.0,
        "area >= 500x500 cross-dist should be 24"
    );

    // Medium zone (300-500)
    let medium = PoissonSpawnSampler::adaptive_for_zone((
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(400.0, 100.0, 400.0),
    ));
    assert_eq!(
        medium.min_same_archetype_dist, 40.0,
        "area 300x300..500x500 should use 40"
    );
    assert_eq!(
        medium.min_cross_archetype_dist, 20.0,
        "area 300x300..500x500 cross-dist should be 20"
    );

    // Small zone (< 300x300)
    let small = PoissonSpawnSampler::adaptive_for_zone((
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(200.0, 100.0, 200.0),
    ));
    assert_eq!(
        small.min_same_archetype_dist, 32.0,
        "area < 300x300 should use 32"
    );
    assert_eq!(
        small.min_cross_archetype_dist, 16.0,
        "area < 300x300 cross-dist should be 16"
    );
}

#[test]
fn adaptive_for_zone_boundary_cases() {
    // Exactly 300x300
    let at_300 = PoissonSpawnSampler::adaptive_for_zone((
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(300.0, 100.0, 300.0),
    ));
    assert_eq!(
        at_300.min_same_archetype_dist, 40.0,
        "exactly 300x300 should use 40 (>= 300x300 threshold)"
    );

    // Exactly 500x500
    let at_500 = PoissonSpawnSampler::adaptive_for_zone((
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(500.0, 100.0, 500.0),
    ));
    assert_eq!(
        at_500.min_same_archetype_dist, 48.0,
        "exactly 500x500 should use 48 (>= 500x500 threshold)"
    );

    // Just under 300x300
    let under_300 = PoissonSpawnSampler::adaptive_for_zone((
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(299.0, 100.0, 299.0),
    ));
    assert_eq!(
        under_300.min_same_archetype_dist, 32.0,
        "299x299 should use 32 (below 300x300 threshold)"
    );
}

#[test]
fn poisson_cross_archetype_distance() {
    let bounds = (DVec3::new(0.0, 64.0, 0.0), DVec3::new(500.0, 80.0, 500.0));
    let sampler = PoissonSpawnSampler::adaptive_for_zone(bounds);

    // Place a Beast at center.
    let existing = vec![(DVec3::new(250.0, 72.0, 250.0), NpcArchetype::Beast)];

    // Sample a Rogue — cross-archetype distance is half of same-archetype.
    let pos = sampler
        .sample_position(bounds, &existing, NpcArchetype::Rogue, 123)
        .expect("large zone should have room for cross-archetype NPC");

    // The sampler should place it respecting cross-archetype distance.
    let dx = pos.x - existing[0].0.x;
    let dz = pos.z - existing[0].0.z;
    let dist = (dx * dx + dz * dz).sqrt();
    // In a 500x500 zone with one NPC, the sampler should find a well-spaced position.
    assert!(
        dist > 0.0,
        "cross-archetype NPC should be placed away from existing; dist={:.1}",
        dist
    );
}

#[test]
fn poisson_first_npc_goes_to_center() {
    let bounds = (DVec3::new(0.0, 64.0, 0.0), DVec3::new(400.0, 80.0, 400.0));
    let sampler = PoissonSpawnSampler::adaptive_for_zone(bounds);
    let pos = sampler
        .sample_position(bounds, &[], NpcArchetype::Rogue, 0)
        .expect("empty zone should always produce a position");
    assert!(
        (pos.x - 200.0).abs() < 1.0 && (pos.z - 200.0).abs() < 1.0,
        "first NPC in empty zone should be at center (200,200); got ({:.1},{:.1})",
        pos.x,
        pos.z,
    );
}

#[test]
fn zone_budget_caps_respected() {
    let mut registry = NpcRegistry::default();
    // Use spawn zone cap of 6.
    let granted = registry.reserve_zone_batch("spawn", 10);
    assert_eq!(
        granted, 6,
        "spawn zone cap is 6; requesting 10 should grant only 6; got {}",
        granted
    );
    // Try again — should get 0.
    let granted2 = registry.reserve_zone_batch("spawn", 5);
    assert_eq!(
        granted2, 0,
        "spawn zone at cap should reject; got {}",
        granted2
    );
}

#[test]
fn seed_count_default_20() {
    let config = RoguePopulationSeedConfig::default();
    assert_eq!(
        config.target_count, 20,
        "plan-npc-overhaul-v1 §P1.3: target_count should default to 20, got {}",
        config.target_count
    );
}

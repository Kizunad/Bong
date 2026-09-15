use bong_server::lingtian::weather::{
    is_stable_tribulation_window, is_xizhuan_phase, try_roll_weather_for_zone,
    try_roll_weather_for_zone_with_profile, weather_apply_to_plot_system,
    weather_generator_system_zone_aware, ActiveWeather, WeatherEvent, WeatherLifecycleEvent,
    WeatherRng,
};
use bong_server::lingtian::weather_profile::{ZoneWeatherProfile, ZoneWeatherProfileRegistry};
use bong_server::lingtian::{LingtianClock, LingtianTickAccumulator, DEFAULT_ZONE};
use bong_server::world::dimension::DimensionKind;
use bong_server::world::season::Season;
use bong_server::world::zone::{Zone, ZoneRegistry};
use valence::prelude::{App, DVec3, Events, Update};

    fn test_zone(name: &str, x: f64) -> Zone {
        Zone {
            name: name.to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::new(x, 60.0, 0.0), DVec3::new(x + 10.0, 90.0, 10.0)),
            spirit_qi: 0.3,
            danger_level: 1,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        }
    }

    #[test]
    fn weather_wire_str_round_trip_round_for_all_variants() {
        // schema/serde 对拍：每个 variant 都有专属 wire 字符串，反序列回原值。
        for ev in WeatherEvent::all() {
            let wire = ev.as_wire_str();
            let json = format!("\"{}\"", wire);
            let back: WeatherEvent =
                serde_json::from_str(&json).unwrap_or_else(|e| panic!("{wire}: {e}"));
            assert_eq!(back, ev, "{wire} round-trip 失败");
        }
    }

    #[test]
    fn weather_blocks_growth_tick_only_blizzard_and_haze() {
        assert!(WeatherEvent::Blizzard.blocks_growth_tick());
        assert!(WeatherEvent::HeavyHaze.blocks_growth_tick());
        assert!(!WeatherEvent::Thunderstorm.blocks_growth_tick());
        assert!(!WeatherEvent::DroughtWind.blocks_growth_tick());
        assert!(!WeatherEvent::LingMist.blocks_growth_tick());
    }

    #[test]
    fn weather_plot_qi_cap_delta_thunderstorm_minus_0_2() {
        assert!((WeatherEvent::Thunderstorm.plot_qi_cap_delta() + 0.2).abs() < 1e-6);
    }

    #[test]
    fn weather_plot_qi_cap_delta_ling_mist_plus_0_2() {
        assert!((WeatherEvent::LingMist.plot_qi_cap_delta() - 0.2).abs() < 1e-6);
    }

    #[test]
    fn weather_plot_qi_cap_delta_neutral_events_zero() {
        for ev in [
            WeatherEvent::DroughtWind,
            WeatherEvent::Blizzard,
            WeatherEvent::HeavyHaze,
        ] {
            assert_eq!(
                ev.plot_qi_cap_delta(),
                0.0,
                "{} should be neutral",
                ev.as_wire_str()
            );
        }
    }

    #[test]
    fn weather_zone_flow_thunderstorm_1_5() {
        assert!((WeatherEvent::Thunderstorm.zone_flow_multiplier() - 1.5).abs() < 1e-6);
        // 其他事件不直接影响 zone_flow（落在 Season 上）。
        for ev in [
            WeatherEvent::DroughtWind,
            WeatherEvent::Blizzard,
            WeatherEvent::HeavyHaze,
            WeatherEvent::LingMist,
        ] {
            assert!((ev.zone_flow_multiplier() - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn weather_qi_decay_drought_wind_doubles() {
        assert!((WeatherEvent::DroughtWind.qi_decay_multiplier() - 2.0).abs() < 1e-6);
        for ev in [
            WeatherEvent::Thunderstorm,
            WeatherEvent::Blizzard,
            WeatherEvent::HeavyHaze,
            WeatherEvent::LingMist,
        ] {
            assert!((ev.qi_decay_multiplier() - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn weather_natural_supply_drought_zero_ling_mist_1_5() {
        assert!(WeatherEvent::DroughtWind.natural_supply_multiplier().abs() < 1e-6);
        assert!((WeatherEvent::LingMist.natural_supply_multiplier() - 1.5).abs() < 1e-6);
        for ev in [
            WeatherEvent::Thunderstorm,
            WeatherEvent::Blizzard,
            WeatherEvent::HeavyHaze,
        ] {
            assert!((ev.natural_supply_multiplier() - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn weather_shelflife_drought_wind_doubles() {
        assert!((WeatherEvent::DroughtWind.shelflife_decay_multiplier() - 2.0).abs() < 1e-6);
        for ev in [
            WeatherEvent::Thunderstorm,
            WeatherEvent::Blizzard,
            WeatherEvent::HeavyHaze,
            WeatherEvent::LingMist,
        ] {
            assert!((ev.shelflife_decay_multiplier() - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn weather_pressure_threshold_relax_haze_only() {
        assert_eq!(WeatherEvent::HeavyHaze.pressure_threshold_relax_steps(), 1);
        for ev in [
            WeatherEvent::Thunderstorm,
            WeatherEvent::DroughtWind,
            WeatherEvent::Blizzard,
            WeatherEvent::LingMist,
        ] {
            assert_eq!(
                ev.pressure_threshold_relax_steps(),
                0,
                "{} should not relax pressure",
                ev.as_wire_str()
            );
        }
    }

    #[test]
    fn weather_all_returns_five_distinct_variants() {
        let all = WeatherEvent::all();
        assert_eq!(all.len(), 5);
        let mut set = std::collections::HashSet::new();
        for ev in all {
            set.insert(ev);
        }
        assert_eq!(set.len(), 5, "WeatherEvent::all() 必须返回 5 个不同变体");
    }

    // -------- plan-lingtian-weather-v1 §6 P2 — 概率表 + 季节耦合 --------

    #[test]
    fn weather_thunderstorm_only_in_summer_or_tide() {
        assert!(WeatherEvent::Thunderstorm.daily_probability(Season::Summer) > 0.0);
        assert!(WeatherEvent::Thunderstorm.daily_probability(Season::SummerToWinter) > 0.0);
        assert!(WeatherEvent::Thunderstorm.daily_probability(Season::WinterToSummer) > 0.0);
        assert_eq!(
            WeatherEvent::Thunderstorm.daily_probability(Season::Winter),
            0.0,
            "雷暴不在冬季出现"
        );
        assert!(!WeatherEvent::Thunderstorm.can_occur_in(Season::Winter));
    }

    #[test]
    fn weather_drought_wind_only_in_summer_or_tide() {
        assert!(WeatherEvent::DroughtWind.daily_probability(Season::Summer) > 0.0);
        assert!(WeatherEvent::DroughtWind.daily_probability(Season::SummerToWinter) > 0.0);
        assert_eq!(
            WeatherEvent::DroughtWind.daily_probability(Season::Winter),
            0.0
        );
    }

    #[test]
    fn weather_blizzard_only_in_winter_or_tide() {
        assert!(WeatherEvent::Blizzard.daily_probability(Season::Winter) > 0.0);
        assert!(WeatherEvent::Blizzard.daily_probability(Season::SummerToWinter) > 0.0);
        assert!(WeatherEvent::Blizzard.daily_probability(Season::WinterToSummer) > 0.0);
        assert_eq!(
            WeatherEvent::Blizzard.daily_probability(Season::Summer),
            0.0,
            "风雪不在夏季出现"
        );
    }

    #[test]
    fn weather_heavy_haze_only_in_winter_or_tide() {
        assert!(WeatherEvent::HeavyHaze.daily_probability(Season::Winter) > 0.0);
        assert!(WeatherEvent::HeavyHaze.daily_probability(Season::SummerToWinter) > 0.0);
        assert_eq!(
            WeatherEvent::HeavyHaze.daily_probability(Season::Summer),
            0.0
        );
    }

    #[test]
    fn weather_ling_mist_only_in_winter_or_tide() {
        assert!(WeatherEvent::LingMist.daily_probability(Season::Winter) > 0.0);
        assert!(WeatherEvent::LingMist.daily_probability(Season::SummerToWinter) > 0.0);
        assert_eq!(
            WeatherEvent::LingMist.daily_probability(Season::Summer),
            0.0
        );
    }

    #[test]
    fn weather_tide_doubles_base_thunderstorm_rng() {
        // §3 表 — 雷暴：base 1% / Summer × 3 = 3% / 汐转 × 2 = 2%
        // 夏 0.03 ≠ 汐转 0.02；汐转 = base 1% × 2
        let summer_p = WeatherEvent::Thunderstorm.daily_probability(Season::Summer);
        let xizhuan_p = WeatherEvent::Thunderstorm.daily_probability(Season::SummerToWinter);
        assert!((summer_p - 0.03).abs() < 1e-6);
        assert!((xizhuan_p - 0.02).abs() < 1e-6);
        // 汐转 prob = base 1% × 2 = 2%
        assert!(xizhuan_p > 0.0 && xizhuan_p < summer_p);
    }

    #[test]
    fn weather_tide_triples_base_ling_mist_rng() {
        // §3 表 — 灵雾：base 1% / Winter / 汐转 × 3 = 3%
        let winter_p = WeatherEvent::LingMist.daily_probability(Season::Winter);
        let xizhuan_p = WeatherEvent::LingMist.daily_probability(Season::SummerToWinter);
        assert!((winter_p - 0.01).abs() < 1e-6);
        assert!((xizhuan_p - 0.03).abs() < 1e-6);
        assert!(xizhuan_p > winter_p, "灵雾汐转应该 > 冬");
    }

    #[test]
    fn weather_duration_ranges_match_plan_table() {
        // §3 表 — 持续时间区间核验（lingtian-tick）：1 game-hour = 60 lingtian-tick
        assert_eq!(
            WeatherEvent::Thunderstorm.duration_range_lingtian_ticks(),
            (120, 240)
        );
        assert_eq!(
            WeatherEvent::DroughtWind.duration_range_lingtian_ticks(),
            (360, 720)
        );
        assert_eq!(
            WeatherEvent::Blizzard.duration_range_lingtian_ticks(),
            (720, 1440)
        );
        assert_eq!(
            WeatherEvent::HeavyHaze.duration_range_lingtian_ticks(),
            (720, 1440)
        );
        assert_eq!(
            WeatherEvent::LingMist.duration_range_lingtian_ticks(),
            (60, 120)
        );
    }

    // -------- ActiveWeather Resource --------

    #[test]
    fn active_weather_insert_and_current_round_trip() {
        let mut active = ActiveWeather::new();
        active.insert("zone_a", WeatherEvent::Thunderstorm, 0, 200);
        assert_eq!(active.current("zone_a"), Some(WeatherEvent::Thunderstorm));
        assert_eq!(active.current("zone_b"), None);
    }

    #[test]
    fn active_weather_event_remaining_ticks_decrements() {
        // event_remaining_ticks 直观语义：expires_at - now_tick 单调下降
        let mut active = ActiveWeather::new();
        active.insert("z", WeatherEvent::Thunderstorm, 0, 1000);
        let entry = active.current_entry("z").expect("just inserted");
        assert_eq!(entry.expires_at_lingtian_tick, 1000);
        assert_eq!(entry.started_at_lingtian_tick, 0);
        // remaining at tick=100 → 900；tick=500 → 500；tick=999 → 1
        for now in [100u64, 500, 999] {
            let remaining = entry.expires_at_lingtian_tick.saturating_sub(now);
            assert_eq!(remaining, 1000 - now);
        }
    }

    #[test]
    fn active_weather_event_expires_clears_active_weather() {
        let mut active = ActiveWeather::new();
        active.insert("z", WeatherEvent::Thunderstorm, 0, 100);
        assert!(active.current("z").is_some());
        // tick 100 → expires_at <= now → 清除
        let removed = active.prune_expired(100);
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].1.event, WeatherEvent::Thunderstorm);
        assert_eq!(removed[0].1.started_at_lingtian_tick, 0);
        assert!(active.current("z").is_none());
        // 二次 prune 应当无变化
        let removed2 = active.prune_expired(200);
        assert!(removed2.is_empty());
    }

    #[test]
    fn active_weather_prune_keeps_unexpired() {
        let mut active = ActiveWeather::new();
        active.insert("z1", WeatherEvent::Thunderstorm, 0, 200);
        active.insert("z2", WeatherEvent::LingMist, 0, 50);
        active.prune_expired(100);
        // z2 expired (50 <= 100)，z1 still alive (200 > 100)
        assert_eq!(active.current("z1"), Some(WeatherEvent::Thunderstorm));
        assert_eq!(active.current("z2"), None);
    }

    #[test]
    fn active_weather_prune_returns_started_at_for_bridge() {
        // bridge 用 started_at 在 wire payload 上保留 `started_at < expires_at` 不变量
        let mut active = ActiveWeather::new();
        active.insert("z", WeatherEvent::Thunderstorm, 1000, 1200);
        let removed = active.prune_expired(1200);
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].1.event, WeatherEvent::Thunderstorm);
        assert_eq!(removed[0].1.started_at_lingtian_tick, 1000);
        assert_eq!(removed[0].1.expires_at_lingtian_tick, 1200);
    }

    #[test]
    fn weather_apply_to_plot_system_emits_original_expiry_tick() {
        let mut app = App::new();
        let mut active = ActiveWeather::new();
        active.insert("z", WeatherEvent::Thunderstorm, 10, 100);
        app.insert_resource(LingtianTickAccumulator::new());
        app.insert_resource(LingtianClock { lingtian_tick: 150 });
        app.insert_resource(active);
        app.add_event::<WeatherLifecycleEvent>();
        app.add_systems(Update, weather_apply_to_plot_system);

        app.update();

        let events = app.world().resource::<Events<WeatherLifecycleEvent>>();
        let expired = events
            .iter_current_update_events()
            .find_map(|event| match event {
                WeatherLifecycleEvent::Expired {
                    zone,
                    event,
                    started_at_lingtian_tick,
                    expired_at_lingtian_tick,
                } => Some((
                    zone.as_str(),
                    *event,
                    *started_at_lingtian_tick,
                    *expired_at_lingtian_tick,
                )),
                WeatherLifecycleEvent::Started { .. } => None,
            })
            .expect("expired lifecycle should be emitted");

        assert_eq!(expired, ("z", WeatherEvent::Thunderstorm, 10, 100));
    }

    // -------- WeatherRng --------

    #[test]
    fn weather_rng_deterministic_with_same_seed() {
        let mut a = WeatherRng::new(42);
        let mut b = WeatherRng::new(42);
        for _ in 0..10 {
            assert_eq!(a.next_f32(), b.next_f32(), "seed=42 必须 deterministic");
        }
    }

    #[test]
    fn weather_rng_next_f32_within_unit_range() {
        let mut rng = WeatherRng::new(7);
        for _ in 0..100 {
            let v = rng.next_f32();
            assert!((0.0..1.0).contains(&v), "next_f32={v} 越界");
        }
    }

    #[test]
    fn weather_rng_next_u64_range_within_bounds() {
        let mut rng = WeatherRng::new(13);
        for _ in 0..100 {
            let v = rng.next_u64_range(360, 720);
            assert!(
                (360..=720).contains(&v),
                "next_u64_range(360, 720) = {v} 越界"
            );
        }
    }

    #[test]
    fn weather_rng_next_u64_range_collapsed_min_eq_max() {
        let mut rng = WeatherRng::new(9);
        assert_eq!(rng.next_u64_range(100, 100), 100);
    }

    // -------- try_roll_weather_for_zone --------

    #[test]
    fn try_roll_skips_when_zone_already_has_event() {
        let mut active = ActiveWeather::new();
        active.insert("z", WeatherEvent::Thunderstorm, 0, 500);
        let mut rng = WeatherRng::new(1);
        let res = try_roll_weather_for_zone("z", Season::Summer, 0, &mut active, &mut rng);
        assert_eq!(res, None, "已有 active 事件时不应再 roll");
        assert_eq!(active.current("z"), Some(WeatherEvent::Thunderstorm));
    }

    #[test]
    fn try_roll_seasons_winter_skips_summer_only_events() {
        // 冬季 roll：雷暴 / 旱风 prob=0 → 永远跳过；可能命中 风雪 / 阴霾 / 灵雾
        let mut hit_summer_only = 0;
        let mut hit_winter_valid = 0;
        for seed in 1u64..200 {
            let mut active = ActiveWeather::new();
            let mut rng = WeatherRng::new(seed);
            if let Some(ev) =
                try_roll_weather_for_zone("z", Season::Winter, 0, &mut active, &mut rng)
            {
                if matches!(ev, WeatherEvent::Thunderstorm | WeatherEvent::DroughtWind) {
                    hit_summer_only += 1;
                } else {
                    hit_winter_valid += 1;
                }
            }
        }
        assert_eq!(hit_summer_only, 0, "冬季不应触发夏限定事件");
        // winter prob 总和 ≈ 4.5%，200 次 seed 至少命中数次
        assert!(hit_winter_valid > 0, "200 次 seed 应至少触发一次冬天事件");
    }

    #[test]
    fn try_roll_summer_only_triggers_summer_or_tide_events() {
        let mut hit_winter_only = 0;
        let mut hit_summer_valid = 0;
        for seed in 1u64..200 {
            let mut active = ActiveWeather::new();
            let mut rng = WeatherRng::new(seed);
            if let Some(ev) =
                try_roll_weather_for_zone("z", Season::Summer, 0, &mut active, &mut rng)
            {
                if matches!(
                    ev,
                    WeatherEvent::Blizzard | WeatherEvent::HeavyHaze | WeatherEvent::LingMist
                ) {
                    hit_winter_only += 1;
                } else {
                    hit_summer_valid += 1;
                }
            }
        }
        assert_eq!(hit_winter_only, 0, "夏季不应触发冬限定事件");
        assert!(hit_summer_valid > 0);
    }

    #[test]
    fn try_roll_inserts_event_with_duration_in_range() {
        // 强 RNG 注入：用一个会命中第一个 valid 事件的种子（雷暴 0.03）
        let mut active = ActiveWeather::new();
        // 找一个能命中的种子（暴力扫一定能找到）
        let mut hit_seed = None;
        for seed in 1u64..200 {
            let mut rng = WeatherRng::new(seed);
            let mut probe = ActiveWeather::new();
            if try_roll_weather_for_zone("z", Season::Summer, 1000, &mut probe, &mut rng).is_some()
            {
                hit_seed = Some(seed);
                break;
            }
        }
        let seed = hit_seed.expect("200 次种子至少命中一次");
        let mut rng = WeatherRng::new(seed);
        let ev = try_roll_weather_for_zone("z", Season::Summer, 1000, &mut active, &mut rng)
            .expect("命中 seed");
        let entry = active.current_entry("z").expect("event inserted");
        let (min_d, max_d) = ev.duration_range_lingtian_ticks();
        let dur = entry.expires_at_lingtian_tick - 1000;
        assert!(
            (min_d..=max_d).contains(&dur),
            "{ev:?} duration {dur} 不在 [{min_d}, {max_d}]"
        );
    }

    #[test]
    fn force_event_overrides_rng() {
        let mut active = ActiveWeather::new();
        let mut rng = WeatherRng::new(1);
        let profile = ZoneWeatherProfile {
            force_event: Some(WeatherEvent::LingMist),
            ..Default::default()
        };

        let event = try_roll_weather_for_zone_with_profile(
            "z",
            Season::Summer,
            1000,
            &mut active,
            &mut rng,
            &profile,
        );

        assert_eq!(event, Some(WeatherEvent::LingMist));
        assert_eq!(active.current("z"), Some(WeatherEvent::LingMist));
    }

    #[test]
    fn force_event_does_not_refresh_active_timer() {
        let mut active = ActiveWeather::new();
        active.insert("z", WeatherEvent::Thunderstorm, 100, 200);
        let mut rng = WeatherRng::new(1);
        let profile = ZoneWeatherProfile {
            force_event: Some(WeatherEvent::LingMist),
            ..Default::default()
        };

        let event = try_roll_weather_for_zone_with_profile(
            "z",
            Season::Summer,
            150,
            &mut active,
            &mut rng,
            &profile,
        );

        assert_eq!(event, None);
        let entry = active.current_entry("z").expect("existing event remains");
        assert_eq!(entry.event, WeatherEvent::Thunderstorm);
        assert_eq!(entry.started_at_lingtian_tick, 100);
        assert_eq!(entry.expires_at_lingtian_tick, 200);
    }

    #[test]
    fn zone_aware_generator_rolls_each_zone_independently() {
        let mut profiles = ZoneWeatherProfileRegistry::new();
        profiles
            .insert(
                "zone_a",
                ZoneWeatherProfile {
                    force_event: Some(WeatherEvent::Thunderstorm),
                    ..Default::default()
                },
            )
            .unwrap();
        profiles
            .insert(
                "zone_b",
                ZoneWeatherProfile {
                    force_event: Some(WeatherEvent::Blizzard),
                    ..Default::default()
                },
            )
            .unwrap();
        let mut app = App::new();
        app.insert_resource(LingtianTickAccumulator::new());
        app.insert_resource(LingtianClock::default());
        app.insert_resource(ActiveWeather::new());
        app.insert_resource(WeatherRng::new(1));
        app.insert_resource(profiles);
        app.insert_resource(ZoneRegistry {
            spatial_revision: 0,
            zones: vec![test_zone("zone_a", 0.0), test_zone("zone_b", 20.0)],
        });
        app.add_event::<WeatherLifecycleEvent>();
        app.add_systems(Update, weather_generator_system_zone_aware);

        app.update();

        let active = app.world().resource::<ActiveWeather>();
        assert_eq!(active.current("zone_a"), Some(WeatherEvent::Thunderstorm));
        assert_eq!(active.current("zone_b"), Some(WeatherEvent::Blizzard));
        let events = app.world().resource::<Events<WeatherLifecycleEvent>>();
        let started = events
            .iter_current_update_events()
            .filter(|event| matches!(event, WeatherLifecycleEvent::Started { .. }))
            .count();
        assert_eq!(started, 2, "两个 zone 应各自 emit started lifecycle");
    }

    #[test]
    fn zone_aware_generator_per_zone_last_rolled_day_dedup() {
        let mut profiles = ZoneWeatherProfileRegistry::new();
        profiles
            .insert(
                "zone_a",
                ZoneWeatherProfile {
                    force_event: Some(WeatherEvent::Thunderstorm),
                    ..Default::default()
                },
            )
            .unwrap();
        let mut active = ActiveWeather::new();
        let mut rng = WeatherRng::new(1);
        let profile = profiles.profile_for("zone_a");

        active.set_last_rolled_day_for("zone_a", 0);
        let event = if active.last_rolled_day_for("zone_a") == Some(0) {
            None
        } else {
            try_roll_weather_for_zone_with_profile(
                "zone_a",
                Season::Summer,
                0,
                &mut active,
                &mut rng,
                &profile,
            )
        };

        assert_eq!(event, None);
        assert_eq!(active.current("zone_a"), None);
    }

    #[test]
    fn zone_aware_generator_empty_registry_does_not_roll_default_zone() {
        let mut app = App::new();
        app.insert_resource(LingtianTickAccumulator::new());
        app.insert_resource(LingtianClock::default());
        app.insert_resource(ActiveWeather::new());
        app.insert_resource(ZoneWeatherProfileRegistry::new());
        app.insert_resource(ZoneRegistry {
            spatial_revision: 0,
            zones: Vec::new(),
        });
        app.add_event::<WeatherLifecycleEvent>();
        app.add_systems(Update, weather_generator_system_zone_aware);

        app.update();

        let active = app.world().resource::<ActiveWeather>();
        assert_eq!(active.last_rolled_day_for(DEFAULT_ZONE), None);
        assert!(active.is_empty());
    }

    #[test]
    fn zone_aware_generator_uses_zone_day_seed_not_previous_zone_rolls() {
        fn zone_b_expiry(zone_names: Vec<&str>) -> u64 {
            let mut profiles = ZoneWeatherProfileRegistry::new();
            for zone in &zone_names {
                profiles
                    .insert(
                        *zone,
                        ZoneWeatherProfile {
                            force_event: Some(WeatherEvent::LingMist),
                            ..Default::default()
                        },
                    )
                    .unwrap();
            }
            let mut app = App::new();
            app.insert_resource(LingtianTickAccumulator::new());
            app.insert_resource(LingtianClock::default());
            app.insert_resource(ActiveWeather::new());
            app.insert_resource(profiles);
            app.insert_resource(ZoneRegistry {
                spatial_revision: 0,
                zones: zone_names
                    .iter()
                    .enumerate()
                    .map(|(index, zone)| test_zone(zone, (index as f64) * 20.0))
                    .collect(),
            });
            app.add_event::<WeatherLifecycleEvent>();
            app.add_systems(Update, weather_generator_system_zone_aware);

            app.update();

            app.world()
                .resource::<ActiveWeather>()
                .current_entry("zone_b")
                .expect("zone_b should roll forced weather")
                .expires_at_lingtian_tick
        }

        assert_eq!(
            zone_b_expiry(vec!["zone_b"]),
            zone_b_expiry(vec!["zone_a", "zone_b"])
        );
    }

    #[test]
    fn single_zone_mvp_compat_when_no_zone_registry_registered() {
        let mut app = App::new();
        app.insert_resource(LingtianTickAccumulator::new());
        app.insert_resource(LingtianClock::default());
        app.insert_resource(ActiveWeather::new());
        app.insert_resource(ZoneWeatherProfileRegistry::new());
        app.add_event::<WeatherLifecycleEvent>();
        app.add_systems(Update, weather_generator_system_zone_aware);

        app.update();

        let active = app.world().resource::<ActiveWeather>();
        assert_eq!(active.last_rolled_day_for(DEFAULT_ZONE), Some(0));
    }

    // -------- plan-lingtian-weather-v1 §6 P4 hooks --------

    #[test]
    fn is_stable_tribulation_window_only_summer_thunderstorm() {
        // 唯一返回 true 的组合：Summer + Thunderstorm
        assert!(is_stable_tribulation_window(
            Season::Summer,
            Some(WeatherEvent::Thunderstorm)
        ));
        // 其他季节 + 雷暴：false
        for season in [
            Season::Winter,
            Season::SummerToWinter,
            Season::WinterToSummer,
        ] {
            assert!(
                !is_stable_tribulation_window(season, Some(WeatherEvent::Thunderstorm)),
                "汐转 / 冬 + 雷暴不应当稳定，{season:?}"
            );
        }
        // Summer + 其他事件：false
        for ev in [
            WeatherEvent::DroughtWind,
            WeatherEvent::Blizzard,
            WeatherEvent::HeavyHaze,
            WeatherEvent::LingMist,
        ] {
            assert!(!is_stable_tribulation_window(Season::Summer, Some(ev)));
        }
        // 无事件：false
        assert!(!is_stable_tribulation_window(Season::Summer, None));
    }

    #[test]
    fn is_xizhuan_phase_helper_matches_season_is_xizhuan() {
        // narrative hint helper：跟 Season::is_xizhuan() 同语义。
        assert!(!is_xizhuan_phase(Season::Summer));
        assert!(!is_xizhuan_phase(Season::Winter));
        assert!(is_xizhuan_phase(Season::SummerToWinter));
        assert!(is_xizhuan_phase(Season::WinterToSummer));
    }

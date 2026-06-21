use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnconsumedEventStatus {
    AuditOnly,
    DeferredFollowUp,
    DirectResourceConsumer,
}

#[derive(Debug, Clone, Copy)]
struct UnconsumedEventTriage {
    event: &'static str,
    status: UnconsumedEventStatus,
    reason: &'static str,
    follow_up: &'static str,
}

const INTENTIONAL_UNCONSUMED_EVENTS: &[UnconsumedEventTriage] = &[
    UnconsumedEventTriage {
        event: "ArtifactMeridianCracked",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "炼器经脉裂痕是领域事件，当前 P 只锁 forge 状态；玩家反馈消费尚未接线。",
        follow_up: "forge feedback/narration follow-up",
    },
    UnconsumedEventTriage {
        event: "ArtifactMeridianDepthChanged",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "炼器经脉深度变化已作为领域事件暴露，当前无运行时 reader；后续 UI/记录消费。",
        follow_up: "forge feedback/narration follow-up",
    },
    UnconsumedEventTriage {
        event: "BoneCoinCrafted",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "妖兽骨币铸造结果当前由 inventory 状态和测试验证；事件预留给经济日志/成就反馈。",
        follow_up: "economy telemetry/narration follow-up",
    },
    UnconsumedEventTriage {
        event: "CraftStartedEvent",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "手搓制作开始事件用于后续 agent/client 制作反馈；当前制作闭环由 session 状态驱动。",
        follow_up: "plan-craft-v1 P2/P3",
    },
    UnconsumedEventTriage {
        event: "DigestionOverloadEvent",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "消化过载已触发状态变化，事件尚未接 HUD/音效；避免误判为已消费。",
        follow_up: "consumable feedback follow-up",
    },
    UnconsumedEventTriage {
        event: "FlowFieldPrototype",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "兽潮 P0 原型事件已被 P1 FlowFieldComputeTask 替代为真实消费路径；旧 prototype 待删除或接调试可视化。",
        follow_up: "plan-beast-horde-v1 cleanup",
    },
    UnconsumedEventTriage {
        event: "ForgeProcessingAccepted",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "炼器处理被接受的领域事件当前只用于测试断言；后续接 forge UI/agent 记录。",
        follow_up: "forge feedback/narration follow-up",
    },
    UnconsumedEventTriage {
        event: "FullPowerStrikeKilledEvent",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "全力一击击杀事件缺 narration/HUD reader；战斗效果本身已在 resolve 路径生效。",
        follow_up: "combat kill feedback follow-up",
    },
    UnconsumedEventTriage {
        event: "IdentityCreatedEvent",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "身份命令 P1 暴露事件，当前没有生平记录/社交系统 reader。",
        follow_up: "identity social/life-record follow-up",
    },
    UnconsumedEventTriage {
        event: "IdentitySwitchedEvent",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "身份切换事件当前未被社交/agent 消费；显式登记防止静默丢弃被误认为闭环。",
        follow_up: "identity social/life-record follow-up",
    },
    UnconsumedEventTriage {
        event: "InfluenceChangedEvent",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "领地影响力变更已写明 NPC 态度/叙事消费者为后续 P；当前只有测试 drain。",
        follow_up: "territory NPC reaction/narration follow-up",
    },
    UnconsumedEventTriage {
        event: "MigrationEvent",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "普通动物迁徙事件目前无运行时 reader；兽潮路径已有独立 BeastHordeEvent reader。",
        follow_up: "fauna migration telemetry/visual follow-up",
    },
    UnconsumedEventTriage {
        event: "NpcScheduleChangedEvent",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "NPC 作息变化事件缺社交/观察 reader；当前调度状态直接写组件。",
        follow_up: "npc schedule narration/social follow-up",
    },
    UnconsumedEventTriage {
        event: "QiTransfer",
        status: UnconsumedEventStatus::DirectResourceConsumer,
        reason: "QiTransfer 是守恒审计事件；真实余额由 WorldQiAccount/调用点直接 apply，不能要求 EventReader 消费。",
        follow_up: "无；守恒口径看 WorldQiAccount 与 qi_physics ledger 测试",
    },
    UnconsumedEventTriage {
        event: "SupplyCoffinOpened",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "物资棺打开事件目前由交互路径直接给奖励；事件预留给统计/叙事。",
        follow_up: "supply coffin telemetry/narration follow-up",
    },
    UnconsumedEventTriage {
        event: "SwordBondFormedEvent",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "剑道结契事件当前由组件状态完成 gameplay；HUD/narration reader 尚未接线。",
        follow_up: "sword-path feedback follow-up",
    },
    UnconsumedEventTriage {
        event: "TechniqueLearnedEvent",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "功法学会事件由技能集合状态落地；当前没有成就/叙事 reader。",
        follow_up: "cultivation technique feedback follow-up",
    },
    UnconsumedEventTriage {
        event: "TechniqueMasteredEvent",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "功法精通事件由熟练度状态落地；叙事/提示消费待接。",
        follow_up: "cultivation technique feedback follow-up",
    },
    UnconsumedEventTriage {
        event: "TribulationFled",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "渡劫逃逸事件目前缺运行时 reader；渡劫状态机直接处理主效果。",
        follow_up: "tribulation narration/telemetry follow-up",
    },
    UnconsumedEventTriage {
        event: "TsySpawnResult",
        status: UnconsumedEventStatus::AuditOnly,
        reason: "TSY dev 命令结果事件用于 dev/test 观测，命令调用方通过事件队列读取，不要求生产 reader。",
        follow_up: "无；dev-only audit event",
    },
    UnconsumedEventTriage {
        event: "TsyZoneInitialized",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "TSY zone 初始化事件当前没有运行时消费者；后续接区域公告/agent 观察。",
        follow_up: "tsy zone narration follow-up",
    },
    UnconsumedEventTriage {
        event: "TurbulenceFieldDecayed",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "涡流场衰减事件当前只被测试观察；后续接视觉消散/战斗记录。",
        follow_up: "woliu_v2 feedback follow-up",
    },
    UnconsumedEventTriage {
        event: "VoidActionBroadcast",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "化虚行为广播事件已作为跨层信号暴露，当前缺 agent/client reader。",
        follow_up: "void action narration follow-up",
    },
    UnconsumedEventTriage {
        event: "YidaoCastCompleteEvent",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "一刀施放完成事件当前无 reader；施法主路径已生效但反馈链需后续接线。",
        follow_up: "yidao feedback follow-up",
    },
    UnconsumedEventTriage {
        event: "ZoneEnvironmentLifecycleEvent",
        status: UnconsumedEventStatus::DeferredFollowUp,
        reason: "区域环境生命周期事件当前由资源内部 drain；Bevy event 预留给叙事/可视化。",
        follow_up: "zone environment narration/visual follow-up",
    },
];

#[derive(Debug, Default)]
struct EventUsageIndex {
    writers: BTreeMap<String, BTreeSet<String>>,
    readers: BTreeMap<String, BTreeSet<String>>,
}

impl EventUsageIndex {
    fn unconsumed_writers(&self) -> BTreeSet<String> {
        self.writers
            .keys()
            .filter(|event| !self.readers.contains_key(*event))
            .cloned()
            .collect()
    }
}

#[test]
fn event_emitters_have_readers_or_triage_entries() {
    let index = scan_server_event_usage().expect("server/src event usage scan should succeed");
    assert_unconsumed_writers_are_triaged(&index);
}

#[test]
fn triage_entries_are_unique_and_documented() {
    let mut seen = BTreeSet::new();
    for entry in INTENTIONAL_UNCONSUMED_EVENTS {
        assert!(
            seen.insert(entry.event),
            "重复的 unconsumed event triage entry: {}",
            entry.event
        );
        assert!(
            !entry.reason.trim().is_empty(),
            "{} 必须写明未消费理由",
            entry.event
        );
        assert!(
            !entry.follow_up.trim().is_empty(),
            "{} 必须写明 follow-up 或明确无后续",
            entry.event
        );
        if entry.status == UnconsumedEventStatus::DeferredFollowUp {
            assert!(
                !entry.follow_up.starts_with("无"),
                "{} 标为 DeferredFollowUp 时必须指向真实后续",
                entry.event
            );
        }
    }
}

#[test]
fn scanner_handles_lifetimes_paths_and_comments() {
    let source = r#"
        // EventWriter<CommentOnlyEvent>
        fn writer(
            a: valence::prelude::EventWriter<'w, crate::foo::PathEvent>,
            b: EventReader<LocalEvent>,
            c: bevy_ecs::event::EventWriter<SimpleEvent>,
        ) {
            let _s = "EventReader<StringLiteralEvent>";
        }
    "#;

    let cleaned = strip_comments_and_strings(source);
    let writers = extract_event_param_types(&cleaned, "EventWriter");
    let readers = extract_event_param_types(&cleaned, "EventReader");

    assert_eq!(
        writers,
        BTreeSet::from(["PathEvent".to_string(), "SimpleEvent".to_string()])
    );
    assert_eq!(readers, BTreeSet::from(["LocalEvent".to_string()]));
}

#[test]
fn unknown_unconsumed_writer_is_reported() {
    let mut index = EventUsageIndex::default();
    index
        .writers
        .entry("ForgottenEvent".to_string())
        .or_default()
        .insert("synthetic.rs".to_string());

    let err = find_untriaged_unconsumed_writers(&index).expect_err("unknown writer must fail");
    assert!(
        err.contains("ForgottenEvent") && err.contains("synthetic.rs"),
        "error should name event and writer location, got: {err}"
    );
}

#[test]
fn triage_entry_becomes_stale_when_reader_exists() {
    let mut index = EventUsageIndex::default();
    index
        .writers
        .entry("QiTransfer".to_string())
        .or_default()
        .insert("writer.rs".to_string());
    index
        .readers
        .entry("QiTransfer".to_string())
        .or_default()
        .insert("reader.rs".to_string());

    let err = find_stale_triage_entries(&index).expect_err("triage with reader should go stale");
    assert!(
        err.contains("QiTransfer") && err.contains("reader.rs"),
        "stale triage error should name event and reader location, got: {err}"
    );
}

#[test]
fn orphan_triage_entry_is_reported_when_writer_disappears() {
    let index = EventUsageIndex::default();

    let err = find_orphan_triage_entries(&index).expect_err("triage without writer should fail");
    assert!(
        err.contains("ArtifactMeridianCracked"),
        "orphan triage error should name deleted or renamed event, got: {err}"
    );
}

#[test]
fn scanner_handles_nested_generics_and_escaped_strings() {
    let source = r#"
        fn writer(
            a: EventWriter<'w, crate::foo::NestedEvent<Result<u8, ()>, Vec<(u8, u8)>>>,
            b: EventReader<'w, crate::foo::NestedEvent<Result<u8, ()>, Vec<(u8, u8)>>>,
        ) {
            let _s = "escaped \" EventWriter<StringLiteralEvent>";
        }
    "#;

    let cleaned = strip_comments_and_strings(source);
    let writers = extract_event_param_types(&cleaned, "EventWriter");
    let readers = extract_event_param_types(&cleaned, "EventReader");

    assert_eq!(writers, BTreeSet::from(["NestedEvent".to_string()]));
    assert_eq!(readers, BTreeSet::from(["NestedEvent".to_string()]));
    assert_eq!(normalize_event_type("'w"), None);
}

#[test]
fn format_locations_truncates_long_location_sets() {
    let locations = BTreeSet::from([
        "01.rs".to_string(),
        "02.rs".to_string(),
        "03.rs".to_string(),
        "04.rs".to_string(),
        "05.rs".to_string(),
        "06.rs".to_string(),
        "07.rs".to_string(),
    ]);

    let formatted = format_locations(&locations);
    assert!(
        formatted.contains("01.rs") && formatted.contains("06.rs"),
        "formatted locations should include the first six sorted paths, got: {formatted}"
    );
    assert!(
        !formatted.contains("07.rs"),
        "formatted locations should truncate after six paths, got: {formatted}"
    );
}

fn assert_unconsumed_writers_are_triaged(index: &EventUsageIndex) {
    if let Err(error) = find_untriaged_unconsumed_writers(index) {
        panic!("{error}");
    }
    if let Err(error) = find_stale_triage_entries(index) {
        panic!("{error}");
    }
    if let Err(error) = find_orphan_triage_entries(index) {
        panic!("{error}");
    }
}

fn find_untriaged_unconsumed_writers(index: &EventUsageIndex) -> Result<(), String> {
    let triaged = triaged_event_names();
    let unknown: Vec<String> = index
        .unconsumed_writers()
        .into_iter()
        .filter(|event| !triaged.contains(event.as_str()))
        .map(|event| {
            let locations = index
                .writers
                .get(&event)
                .map(format_locations)
                .unwrap_or_else(|| "<unknown>".to_string());
            format!("{event} <- {locations}")
        })
        .collect();

    if unknown.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "发现未 triage 的 EventWriter<T> 且没有 EventReader<T>：\n{}\n\
             处理方式：接入真实 EventReader 消费，或在 INTENTIONAL_UNCONSUMED_EVENTS 写明理由和 follow-up。",
            unknown.join("\n")
        ))
    }
}

fn find_stale_triage_entries(index: &EventUsageIndex) -> Result<(), String> {
    let stale: Vec<String> = INTENTIONAL_UNCONSUMED_EVENTS
        .iter()
        .filter(|entry| index.readers.contains_key(entry.event))
        .map(|entry| {
            let readers = index
                .readers
                .get(entry.event)
                .map(format_locations)
                .unwrap_or_else(|| "<unknown>".to_string());
            format!("{} now has EventReader at {readers}", entry.event)
        })
        .collect();

    if stale.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "以下 unconsumed triage 已过期，应从 INTENTIONAL_UNCONSUMED_EVENTS 移除：\n{}",
            stale.join("\n")
        ))
    }
}

fn find_orphan_triage_entries(index: &EventUsageIndex) -> Result<(), String> {
    let orphaned: Vec<String> = INTENTIONAL_UNCONSUMED_EVENTS
        .iter()
        .filter(|entry| !index.writers.contains_key(entry.event))
        .map(|entry| entry.event.to_string())
        .collect();

    if orphaned.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "以下 unconsumed triage 找不到对应 EventWriter，应删除或更新事件名：\n{}",
            orphaned.join("\n")
        ))
    }
}

fn triaged_event_names() -> BTreeSet<&'static str> {
    INTENTIONAL_UNCONSUMED_EVENTS
        .iter()
        .map(|entry| entry.event)
        .collect()
}

fn format_locations(locations: &BTreeSet<String>) -> String {
    locations
        .iter()
        .take(6)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ")
}

fn scan_server_event_usage() -> std::io::Result<EventUsageIndex> {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rust_files(&src_dir, &mut files)?;

    let mut index = EventUsageIndex::default();
    for path in files {
        if path
            .file_name()
            .is_some_and(|name| name == "test_coverage_guards.rs")
        {
            continue;
        }
        let source = fs::read_to_string(&path)?;
        let cleaned = strip_comments_and_strings(&source);
        let rel_path = path
            .strip_prefix(&src_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        for event in extract_event_param_types(&cleaned, "EventWriter") {
            index
                .writers
                .entry(event)
                .or_default()
                .insert(rel_path.clone());
        }
        for event in extract_event_param_types(&cleaned, "EventReader") {
            index
                .readers
                .entry(event)
                .or_default()
                .insert(rel_path.clone());
        }
    }
    Ok(index)
}

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    Ok(())
}

fn extract_event_param_types(source: &str, marker: &str) -> BTreeSet<String> {
    let mut events = BTreeSet::new();
    let marker_bytes = marker.as_bytes();
    let bytes = source.as_bytes();
    let mut cursor = 0;

    while let Some(relative) = source[cursor..].find(marker) {
        let marker_start = cursor + relative;
        let mut generic_start = marker_start + marker_bytes.len();
        while bytes
            .get(generic_start)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            generic_start += 1;
        }
        if bytes.get(generic_start) != Some(&b'<') {
            cursor = generic_start;
            continue;
        }

        if let Some((generic, end)) = balanced_generic(source, generic_start) {
            if let Some(event) = normalize_event_type(&generic) {
                events.insert(event);
            }
            cursor = end;
        } else {
            cursor = generic_start + 1;
        }
    }

    events
}

fn balanced_generic(source: &str, open_index: usize) -> Option<(String, usize)> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut end = open_index;
    for (offset, byte) in bytes[open_index..].iter().enumerate() {
        match byte {
            b'<' => depth += 1,
            b'>' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    end = open_index + offset;
                    break;
                }
            }
            _ => {}
        }
    }

    if depth == 0 && end > open_index {
        Some((source[open_index + 1..end].to_string(), end + 1))
    } else {
        None
    }
}

fn normalize_event_type(generic: &str) -> Option<String> {
    let event_segment = last_top_level_comma_segment(generic).trim();
    let without_generics = event_segment
        .split('<')
        .next()
        .unwrap_or(event_segment)
        .trim();
    let last_path_segment = without_generics
        .rsplit("::")
        .next()
        .unwrap_or(without_generics)
        .trim();
    let event: String = last_path_segment
        .chars()
        .skip_while(|ch| ch.is_whitespace() || *ch == '&')
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect();

    if event.is_empty() || event.starts_with('\'') {
        None
    } else {
        Some(event)
    }
}

fn last_top_level_comma_segment(generic: &str) -> &str {
    let mut depth = 0usize;
    let mut last_comma = None;
    for (index, ch) in generic.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => last_comma = Some(index),
            _ => {}
        }
    }
    last_comma
        .map(|index| &generic[index + 1..])
        .unwrap_or(generic)
}

fn strip_comments_and_strings(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '/' if chars.peek() == Some(&'/') => {
                chars.next();
                for next in chars.by_ref() {
                    if next == '\n' {
                        output.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut previous = '\0';
                for next in chars.by_ref() {
                    if next == '\n' {
                        output.push('\n');
                    }
                    if previous == '*' && next == '/' {
                        break;
                    }
                    previous = next;
                }
            }
            '"' => {
                output.push('"');
                let mut escaped = false;
                for next in chars.by_ref() {
                    if next == '\n' {
                        output.push('\n');
                    }
                    if next == '"' && !escaped {
                        output.push('"');
                        break;
                    }
                    escaped = next == '\\' && !escaped;
                    if next != '\\' {
                        escaped = false;
                    }
                }
            }
            _ => output.push(ch),
        }
    }

    output
}

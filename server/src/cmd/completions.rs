//! dev 命令参数 Tab 补全 — AskServer 应答端。
//!
//! valence 的命令树把参数节点的 `suggestion` 硬编为 `None`，原版客户端对没有
//! suggestion provider 的 string 参数不发起任何补全请求；且 valence 只定义了
//! `RequestCommandCompletionsC2s` / `CommandSuggestionsS2c` 两个 packet，没有
//! 应答实现。本模块补齐两端：
//!
//! 1. [`mark_ask_server_arguments`]（PostStartup）— 直接改
//!    `CommandRegistry.graph`，把 [`ROUTES`] 命中的参数节点标成
//!    `Suggestion::AskServer`。资源被标 changed 后 valence 的
//!    `update_command_tree` 自动向全体客户端重发命令树。
//! 2. [`answer_command_completions`]（EventLoopPreUpdate）— 收
//!    `RequestCommandCompletionsC2s`，按 [`ROUTES`] 路由到数据源算前缀匹配，
//!    回 `CommandSuggestionsS2c`（悬停 tooltip = 中文 display_name）。
//!
//! offset 语义（对齐 vanilla ServerGamePacketListenerImpl）：客户端发的 text
//! 是输入框光标前的完整内容（含前导 `/`），回包 start/length 是**该字符串内**
//! 待替换片段的字符偏移。

use std::borrow::Cow;

use valence::command::CommandRegistry;
use valence::event_loop::PacketEvent;
use valence::prelude::{Client, EventReader, Query, Res, ResMut};
use valence::protocol::encode::WritePacket;
use valence::protocol::packets::play::command_suggestions_s2c::CommandSuggestionsMatch;
use valence::protocol::packets::play::command_tree_s2c::{NodeData, Suggestion};
use valence::protocol::packets::play::{CommandSuggestionsS2c, RequestCommandCompletionsC2s};
use valence::protocol::VarInt;
use valence::text::Text;

use crate::cultivation::known_techniques::TECHNIQUE_DEFINITIONS;
use crate::inventory::ItemRegistry;
use crate::world::zone::ZoneRegistry;

/// 单条候选：补全串 + 悬停 tooltip（如中文 display_name）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub value: String,
    pub tooltip: Option<String>,
}

impl Candidate {
    fn plain(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            tooltip: None,
        }
    }

    fn with_tooltip(value: impl Into<String>, tooltip: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            tooltip: Some(tooltip.into()),
        }
    }
}

/// 补全数据源种类 — [`ROUTES`] 的路由目标。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionSource {
    /// `ItemRegistry` 全部 template id（tooltip = display_name）
    ItemTemplates,
    /// `TECHNIQUE_DEFINITIONS` 48 功法 id（tooltip = display_name）
    Techniques,
    /// 20 经脉 canonical id（12 正经 + 奇经八脉）
    Meridians,
    /// 6 境界 english id
    Realms,
    /// `ZoneRegistry` zone name
    Zones,
}

/// 路由表：`(命令路径字面量, 参数在命令中的词序, 数据源)`。
///
/// 词序从 0 计（0 = 命令名本身）。例：`/give <id>` 的 id 是第 1 词；
/// `/technique add <id>` 的 id 是第 2 词。命中路径且光标正落在该词上才给建议。
///
/// 加新路由三步：这里加一行 → 确认路径与 `assemble_graph` 的 literal 一致
/// （有 `routes_resolve_against_command_graph` 对拍锁）→ 数据源不在
/// [`CompletionSource`] 就扩 enum + `candidates_for` 分支。
pub const ROUTES: &[(&[&str], usize, CompletionSource)] = &[
    (&["give"], 1, CompletionSource::ItemTemplates),
    (&["technique", "add"], 2, CompletionSource::Techniques),
    (&["technique", "give"], 2, CompletionSource::Techniques),
    (&["technique", "remove"], 2, CompletionSource::Techniques),
    (
        &["technique", "proficiency"],
        2,
        CompletionSource::Techniques,
    ),
    (&["technique", "active"], 2, CompletionSource::Techniques),
    (&["meridian", "open"], 2, CompletionSource::Meridians),
    (&["realm", "set"], 2, CompletionSource::Realms),
    (&["zone_qi", "set"], 2, CompletionSource::Zones),
];

/// 20 经脉 canonical id — 与 `dev::meridian::parse_meridian_id` 的主拼写一致
/// （别名不进补全列表；`meridian_completion_ids_all_parse_and_cover_variants`
/// 双向对拍防漂移）。
pub const MERIDIAN_COMPLETION_IDS: &[&str] = &[
    "lung",
    "large_intestine",
    "stomach",
    "spleen",
    "heart",
    "small_intestine",
    "bladder",
    "kidney",
    "pericardium",
    "triple_energizer",
    "gallbladder",
    "liver",
    "ren",
    "du",
    "chong",
    "dai",
    "yinqiao",
    "yangqiao",
    "yinwei",
    "yangwei",
];

/// 6 境界 english id — 与 `dev::realm::parse_realm` 对拍（worldview 六境界）。
pub const REALM_COMPLETION_IDS: &[(&str, &str)] = &[
    ("awaken", "醒灵"),
    ("induce", "引气"),
    ("condense", "凝脉"),
    ("solidify", "固元"),
    ("spirit", "通灵"),
    ("void", "化虚"),
];

/// 单次回包最多候选数 — 客户端建议框可滚动，但全量物品模板（数百条）没必要
/// 一次全发；键入首字母后立刻收敛。
pub const MAX_SUGGESTIONS: usize = 128;

/// 已解析的补全请求：光标所在词的路由命中 + 待替换片段偏移。
#[derive(Debug, PartialEq, Eq)]
pub struct CompletionQuery {
    pub source: CompletionSource,
    /// 光标所在词已键入的部分（小写化前的原文）。
    pub partial: String,
    /// 待替换片段在客户端原始 text（含 `/`）中的字符偏移。
    pub start: i32,
    pub length: i32,
}

/// 解析客户端发来的补全请求文本 → 路由命中。
///
/// `None` = 不归我们管（非注册命令 / 光标不在已路由的参数位上），静默不回包
/// —— 原版客户端对无响应的请求只是不显示建议，无副作用。
pub fn parse_completion_query(text: &str) -> Option<CompletionQuery> {
    let body = text.strip_prefix('/')?;
    let words: Vec<&str> = body.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }
    // 光标位于第几个词：尾随空白 = 正在开启下一个词（partial 为空）。
    let trailing_space = body.ends_with(char::is_whitespace);
    let (cursor_word_index, partial) = if trailing_space {
        (words.len(), "")
    } else {
        (words.len() - 1, *words.last().expect("words non-empty"))
    };

    let source = ROUTES.iter().find_map(|(path, arg_index, source)| {
        let path_matches = words.len() >= path.len().min(cursor_word_index + 1)
            && cursor_word_index == *arg_index
            && path
                .iter()
                .zip(words.iter())
                .all(|(lit, word)| lit.eq_ignore_ascii_case(word));
        // 路径字面量必须全部已敲完（光标在参数位，字面量数 = arg_index）。
        (path_matches && path.len() == *arg_index).then_some(*source)
    })?;

    let partial_chars = partial.chars().count() as i32;
    let total_chars = text.chars().count() as i32;
    Some(CompletionQuery {
        source,
        partial: partial.to_string(),
        start: total_chars - partial_chars,
        length: partial_chars,
    })
}

/// 按数据源取全量候选（未过滤）。resource 缺席的源返回空。
fn candidates_for(
    source: CompletionSource,
    items: &ItemRegistry,
    zones: Option<&ZoneRegistry>,
) -> Vec<Candidate> {
    match source {
        CompletionSource::ItemTemplates => items
            .iter_templates()
            .map(|t| Candidate::with_tooltip(t.id.clone(), t.display_name.clone()))
            .collect(),
        CompletionSource::Techniques => TECHNIQUE_DEFINITIONS
            .iter()
            .map(|d| Candidate::with_tooltip(d.id, d.display_name))
            .collect(),
        CompletionSource::Meridians => MERIDIAN_COMPLETION_IDS
            .iter()
            .map(|id| Candidate::plain(*id))
            .collect(),
        CompletionSource::Realms => REALM_COMPLETION_IDS
            .iter()
            .map(|(id, label)| Candidate::with_tooltip(*id, *label))
            .collect(),
        CompletionSource::Zones => zones
            .map(|registry| {
                registry
                    .zones
                    .iter()
                    .map(|zone| Candidate::plain(zone.name.clone()))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// 前缀过滤 + 排序 + 截断（大小写不敏感；候选按字典序稳定输出）。
pub fn filter_candidates(mut candidates: Vec<Candidate>, partial: &str) -> Vec<Candidate> {
    let needle = partial.to_ascii_lowercase();
    candidates.retain(|c| c.value.to_ascii_lowercase().starts_with(&needle));
    candidates.sort_by(|a, b| a.value.cmp(&b.value));
    candidates.truncate(MAX_SUGGESTIONS);
    candidates
}

/// PostStartup — 把 [`ROUTES`] 命中的参数节点标成 `AskServer`。
///
/// 所有 `add_command` 在 App 构建期完成，PostStartup 时命令图已定型；改动触发
/// `Res<CommandRegistry>` change detection，valence `update_command_tree` 自动
/// 向全体在线客户端重发命令树（后进玩家走 `Added<Client>` 路径天然拿到）。
pub fn mark_ask_server_arguments(mut registry: ResMut<CommandRegistry>) {
    let marked = mark_routes_ask_server(&mut registry);
    if marked < ROUTES.len() {
        tracing::warn!(
            "[bong][cmd] tab completion: only {marked}/{} routes resolved in command graph — \
             a ROUTES path is out of sync with its assemble_graph literals",
            ROUTES.len()
        );
    } else {
        tracing::info!("[bong][cmd] tab completion armed on {marked} argument nodes");
    }
}

/// 图改写本体（拆出便于测试）。返回成功标记的路由数。
pub fn mark_routes_ask_server(registry: &mut CommandRegistry) -> usize {
    let mut marked = 0;
    for (path, _, _) in ROUTES {
        if with_argument_node_data(&mut registry.graph, path, |data| {
            if let NodeData::Argument { suggestion, .. } = data {
                *suggestion = Some(Suggestion::AskServer);
            }
        }) {
            marked += 1;
        }
    }
    marked
}

/// 从根沿 literal 路径下行到路径尽头的第一个 argument 子节点，对其 NodeData
/// 应用 `f`。返回是否命中（路径或参数节点缺失 = false，不执行 `f`）。
///
/// 不对外暴露 `petgraph::NodeIndex`（petgraph 非本 crate 直接依赖）。
fn with_argument_node_data(
    graph: &mut valence::command::graph::CommandGraph,
    path: &[&str],
    f: impl FnOnce(&mut NodeData),
) -> bool {
    let mut current = graph.root;
    for literal in path {
        let Some(next) = graph.graph.neighbors(current).find(
            |&idx| matches!(&graph.graph[idx].data, NodeData::Literal { name } if name == literal),
        ) else {
            return false;
        };
        current = next;
    }
    let Some(arg) = graph
        .graph
        .neighbors(current)
        .find(|&idx| matches!(&graph.graph[idx].data, NodeData::Argument { .. }))
    else {
        return false;
    };
    f(&mut graph.graph[arg].data);
    true
}

/// 只读查询：路径尽头参数节点当前的 suggestion（`None` = 路径没解析到参数节点）。
pub fn argument_suggestion(
    graph: &valence::command::graph::CommandGraph,
    path: &[&str],
) -> Option<Option<Suggestion>> {
    let mut current = graph.root;
    for literal in path {
        current = graph.graph.neighbors(current).find(
            |&idx| matches!(&graph.graph[idx].data, NodeData::Literal { name } if name == literal),
        )?;
    }
    let arg = graph
        .graph
        .neighbors(current)
        .find(|&idx| matches!(&graph.graph[idx].data, NodeData::Argument { .. }))?;
    match &graph.graph[arg].data {
        NodeData::Argument { suggestion, .. } => Some(*suggestion),
        _ => None,
    }
}

/// EventLoopPreUpdate — 应答客户端补全请求。
pub fn answer_command_completions(
    mut packets: EventReader<PacketEvent>,
    items: Res<ItemRegistry>,
    zones: Option<Res<ZoneRegistry>>,
    mut clients: Query<&mut Client>,
) {
    for packet in packets.read() {
        let Some(request) = packet.decode::<RequestCommandCompletionsC2s>() else {
            continue;
        };
        let Some(query) = parse_completion_query(request.text.0) else {
            continue;
        };
        let Ok(mut client) = clients.get_mut(packet.client) else {
            continue;
        };
        let candidates = filter_candidates(
            candidates_for(query.source, &items, zones.as_deref()),
            &query.partial,
        );
        let tooltips: Vec<Option<Text>> = candidates
            .iter()
            .map(|c| c.tooltip.clone().map(Text::from))
            .collect();
        let matches: Vec<CommandSuggestionsMatch> = candidates
            .iter()
            .zip(tooltips.iter())
            .map(|(c, tooltip)| CommandSuggestionsMatch {
                suggested_match: c.value.as_str(),
                tooltip: tooltip.as_ref().map(Cow::Borrowed),
            })
            .collect();
        client.write_packet(&CommandSuggestionsS2c {
            id: request.transaction_id,
            start: VarInt(query.start),
            length: VarInt(query.length),
            matches,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::meridian::parse_meridian_id;
    use crate::cmd::dev::realm::parse_realm;
    use std::collections::HashSet;

    // ── parse_completion_query：路由 / 词位 / 偏移 ────────────────────

    #[test]
    fn give_first_arg_routes_to_item_templates_with_partial_offsets() {
        let q = parse_completion_query("/give qic").expect("should route");
        assert_eq!(q.source, CompletionSource::ItemTemplates);
        assert_eq!(q.partial, "qic");
        assert_eq!(
            (q.start, q.length),
            (6, 3),
            "start 应指向 partial 词首（`/give ` 共 6 字符），length = partial 长度"
        );
    }

    #[test]
    fn give_trailing_space_yields_empty_partial_at_text_end() {
        let q = parse_completion_query("/give ").expect("should route");
        assert_eq!(q.partial, "");
        assert_eq!((q.start, q.length), (6, 0));
    }

    #[test]
    fn give_second_arg_count_position_not_routed() {
        assert_eq!(
            parse_completion_query("/give qicao_grass 3"),
            None,
            "count 位是 u32，不应给 id 建议"
        );
        assert_eq!(parse_completion_query("/give qicao_grass "), None);
    }

    #[test]
    fn technique_subcommands_route_to_techniques() {
        for sub in ["add", "give", "remove", "proficiency", "active"] {
            let text = format!("/technique {sub} wo");
            let q = parse_completion_query(&text)
                .unwrap_or_else(|| panic!("`{text}` 应命中 Techniques 路由"));
            assert_eq!(q.source, CompletionSource::Techniques);
            assert_eq!(q.partial, "wo");
        }
    }

    #[test]
    fn technique_value_position_after_id_not_routed() {
        // /technique proficiency <id> <value> — value 位是 f64
        assert_eq!(
            parse_completion_query("/technique proficiency woliu 0."),
            None
        );
    }

    #[test]
    fn meridian_realm_zone_routes_resolve() {
        assert_eq!(
            parse_completion_query("/meridian open lu").unwrap().source,
            CompletionSource::Meridians
        );
        assert_eq!(
            parse_completion_query("/realm set vo").unwrap().source,
            CompletionSource::Realms
        );
        assert_eq!(
            parse_completion_query("/zone_qi set sp").unwrap().source,
            CompletionSource::Zones
        );
    }

    #[test]
    fn cursor_on_literal_itself_not_routed() {
        // 光标还在敲字面量（命令名/子命令），字面量补全归客户端本地，不应答。
        for text in [
            "/give",
            "/giv",
            "/technique add",
            "/technique",
            "/meridian open",
        ] {
            assert_eq!(
                parse_completion_query(text),
                None,
                "`{text}` 光标在字面量上，不应路由到参数数据源"
            );
        }
    }

    #[test]
    fn unrelated_or_malformed_text_not_routed() {
        for text in ["", "/", "give qic", "/unknown_cmd foo", "/kill self"] {
            assert_eq!(
                parse_completion_query(text),
                None,
                "`{text}` 不应命中任何路由"
            );
        }
    }

    #[test]
    fn multibyte_prefix_offsets_counted_in_chars() {
        // 客户端 text 是字符流；万一用户在参数位敲中文，偏移必须按字符不按字节。
        let q = parse_completion_query("/realm set 化").expect("should route");
        assert_eq!(q.partial, "化");
        assert_eq!(
            (q.start, q.length),
            (11, 1),
            "start/length 按字符计（`/realm set ` = 11 字符，`化` = 1 字符）"
        );
    }

    // ── filter_candidates：过滤 / 排序 / 截断 ─────────────────────────

    fn plain_candidates(ids: &[&str]) -> Vec<Candidate> {
        ids.iter().map(|id| Candidate::plain(*id)).collect()
    }

    #[test]
    fn filter_is_prefix_match_case_insensitive_sorted() {
        let out = filter_candidates(
            plain_candidates(&["zhu_pi", "Qicao_grass", "qingye_leaf", "fan_tie"]),
            "Qi",
        );
        assert_eq!(
            out.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(),
            vec!["Qicao_grass", "qingye_leaf"],
            "应大小写不敏感前缀匹配并按字典序输出"
        );
    }

    #[test]
    fn filter_empty_partial_returns_all_sorted() {
        let out = filter_candidates(plain_candidates(&["b", "a", "c"]), "");
        assert_eq!(
            out.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(),
            vec!["a", "b", "c"]
        );
    }

    #[test]
    fn filter_no_match_returns_empty() {
        assert!(filter_candidates(plain_candidates(&["a", "b"]), "zzz").is_empty());
    }

    #[test]
    fn filter_truncates_at_max_suggestions() {
        let many: Vec<Candidate> = (0..MAX_SUGGESTIONS + 50)
            .map(|i| Candidate::plain(format!("item_{i:04}")))
            .collect();
        assert_eq!(
            filter_candidates(many, "").len(),
            MAX_SUGGESTIONS,
            "超量候选必须截断到 MAX_SUGGESTIONS，防单包过大"
        );
    }

    // ── 静态候选表 ↔ 命令解析器 双向对拍（防漂移）─────────────────────

    #[test]
    fn meridian_completion_ids_all_parse_and_cover_variants() {
        let mut seen = HashSet::new();
        for id in MERIDIAN_COMPLETION_IDS {
            let parsed = parse_meridian_id(id).unwrap_or_else(|| {
                panic!("补全表经脉 id `{id}` 无法被 parse_meridian_id 解析 —— 两表已漂移")
            });
            assert!(
                seen.insert(parsed),
                "补全表经脉 id `{id}` 与其他条目解析到同一 MeridianId —— 重复条目"
            );
        }
        assert_eq!(
            seen.len(),
            20,
            "补全表应恰好覆盖全部 20 条经脉（12 正经 + 奇经八脉），实际={}",
            seen.len()
        );
    }

    #[test]
    fn realm_completion_ids_all_parse_and_cover_variants() {
        let mut seen = HashSet::new();
        for (id, label) in REALM_COMPLETION_IDS {
            let parsed = parse_realm(id).unwrap_or_else(|| {
                panic!("补全表境界 id `{id}` 无法被 parse_realm 解析 —— 两表已漂移")
            });
            let from_label = parse_realm(label)
                .unwrap_or_else(|| panic!("补全表境界中文名 `{label}` 无法被 parse_realm 解析"));
            assert_eq!(
                parsed, from_label,
                "`{id}` 与中文名 `{label}` 应解析到同一境界"
            );
            assert!(seen.insert(parsed), "境界 id `{id}` 重复");
        }
        assert_eq!(seen.len(), 6, "补全表应恰好覆盖 worldview 全部 6 境界");
    }

    #[test]
    fn technique_source_yields_all_definitions_with_tooltips() {
        let items = ItemRegistry::from_map(Default::default());
        let out = candidates_for(CompletionSource::Techniques, &items, None);
        assert_eq!(out.len(), TECHNIQUE_DEFINITIONS.len());
        assert!(
            out.iter().all(|c| c.tooltip.is_some()),
            "功法候选应带 display_name tooltip"
        );
    }

    #[test]
    fn zones_source_without_registry_is_empty_not_panic() {
        let items = ItemRegistry::from_map(Default::default());
        assert!(candidates_for(CompletionSource::Zones, &items, None).is_empty());
    }

    #[test]
    fn zones_source_lists_registry_names() {
        let items = ItemRegistry::from_map(Default::default());
        let zones = ZoneRegistry::fallback();
        let out = candidates_for(CompletionSource::Zones, &items, Some(&zones));
        assert_eq!(
            out.len(),
            zones.zones.len(),
            "zone 候选数应与 registry 一致"
        );
        assert!(out.iter().any(|c| c.value == zones.zones[0].name));
    }

    // ── 图改写：ROUTES ↔ 真实命令图 对拍 ─────────────────────────────

    #[test]
    fn routes_resolve_against_command_graph() {
        // 起一个真实 App 把全部 dev 命令注册进 CommandRegistry，再跑图改写：
        // 每条 ROUTES 路径都必须解析到一个 argument 节点并成功标记 AskServer。
        // 任一命令的 assemble_graph 改了 literal 名，这里立刻撞红。
        let mut app = crate::cmd::test_command_app();
        let mut registry = app
            .world_mut()
            .remove_resource::<CommandRegistry>()
            .expect("CommandRegistry inserted by command plugin");
        let marked = mark_routes_ask_server(&mut registry);
        assert_eq!(
            marked,
            ROUTES.len(),
            "全部 {} 条路由都应在命令图中解析成功（assemble_graph literal 与 ROUTES 漂移）",
            ROUTES.len()
        );
        // 验证每条路由的参数节点都确实带上了 AskServer。
        for (path, _, _) in ROUTES {
            assert_eq!(
                argument_suggestion(&registry.graph, path),
                Some(Some(Suggestion::AskServer)),
                "路由 {path:?} 的参数节点应被标为 AskServer"
            );
        }
    }
}

# proto→JSON 桥接契约审计 — CONFIRMED findings

审计面 124 契约 · 原始 97 findings → **CONFIRMED 73**(critical 43 / warn 22 / info 8) · REFUTED 24 · clean 71

> 根因:线上 server 走 proto,客户端 ProtoServerDataBridge 仅 14 个 payloadCase 有专属 fixup,其余走通用 JsonFormat.printer(preservingProtoFieldNames+includingDefaultValueFields)。凡 handler 读复合字段/枚举小写比较/读拍平坐标,而 proto3 JSON 实际输出 flat/枚举全名/字符串化 uint64,即静默 null/noOp,无异常无日志。

## 按根因聚类(修复应按 cluster 批量处理)

- **RC2 枚举名未剥前缀 (proto 输出 ENUM_FULL_NAME vs handler 期望 snake_case)** — 34 条(crit 20/warn 11/info 3):social_exposure, rift_portal_state, search_aborted, yidao_hud_state, skill_xp_gain, player_state, alchemy_outcome_resolved, forge_session, alchemy_contamination, botany_plant_v2_render_profiles, gathering_session, lingtian_session, carrier_state, false_skin_state, qi_color_observed, spiritual_sense_targets, event_alert, skill_lv_up, skill_cap_changed, skill_scroll_used, extract_started, extract_aborted, extract_failed, container_state, inventory_snapshot, forge_outcome, realm_vision_params, spirit_treasure_dialogue, craft_outcome
- **RC3 坐标被拍平 (world_pos_x/y/z / pos_x/y/z) vs handler 读 world_pos 数组** — 11 条(crit 10/warn 1/info 0):trade_offer, rift_portal_state, container_state, alchemy_furnace, botany_harvest_progress, lingtian_session, breakthrough_cinematic, inventory_event, dropped_loot_sync, mining_progress
- **RC1 uint64→JSON字符串, readLong 的 isNumber() 门恒 null** — 14 条(crit 8/warn 5/info 1):social_pact, sparring_invite, trade_offer, poison_overdose_event, loot_container_update, defense_window, death_screen, social_renown_delta, niche_intrusion, full_power_exhausted_state, wounds_snapshot, false_skin_state, recipe_unlocked
- **RC6 字段名漂移/不存在** — 12 条(crit 3/warn 5/info 4):craft_recipe_list, event_alert, ui_open, player_state, techniques_snapshot, combat_event, heart_demon_offer, zone_info, craft_session_state
- **RC4 proto string 内嵌 JSON (双重编码) vs handler 读对象** — 1 条(crit 1/warn 0/info 0):loot_container_open
- **RC5 其他形状不符 (数组/对象/map/元组)** — 1 条(crit 1/warn 0/info 0):recipe_unlocked

---

## RC2 枚举名未剥前缀 (proto 输出 ENUM_FULL_NAME vs handler 期望 snake_case)  · 34 条

### [CRITICAL] `social_exposure` · `kind` (B_enum_case)
- **proto 实际输出**:enum ExposureKind → JSON 全名字符串,如 "EXPOSURE_KIND_CHAT"/"EXPOSURE_KIND_TRADE"/"EXPOSURE_KIND_DIVINE"/"EXPOSURE_KIND_DEATH"（preservingProtoFieldNames 不影响 enum 值本身）
- **handler 期望**:EXPOSURE_KINDS = Set.of("chat","trade","divine","death") 做无前缀小写字面量 equals 比对
- **运行时后果**:kind 永远不在 EXPOSURE_KINDS 集合中 → 第 67 行条件恒真 → social_exposure 消息永远走 noOp 分支，SocialStateStore.recordExposure 从未被调用，身份暴露 HUD 提示永不触发
- **证据**:client/src/main/java/com/bong/client/network/SocialServerDataHandler.java:64,67 — String kind = readString(p, "kind"); ... !EXPOSURE_KINDS.contains(kind)  |  proto/bong/envelope.proto:1988-1994 (enum ExposureKind), 2041-2047 (message SocialExposure, kind=2)
- **修法**:generic path 加 stripEnumPrefix("EXPOSURE_KIND_") 再小写，或给 social_exposure 加专属 fixup 方法把 kind 规范化

### [CRITICAL] `rift_portal_state` · `direction` (B_enum_case)
- **proto 实际输出**:Enum full-name string per JsonFormat canonical output, e.g. "RIFT_PORTAL_DIRECTION_EXIT" / "RIFT_PORTAL_DIRECTION_ENTRY" / "RIFT_PORTAL_DIRECTION_UNSPECIFIED".
- **handler 期望**:Lowercase unprefixed literal "exit", compared with String.equals in downstream consumers.
- **运行时后果**:Even if the world_pos bug above were fixed so portals registered, ExtractStateStore.nearestPortal() would still always return null because no portal's direction literal-equals "exit". ExtractInteractionBootstrap.java:37-41 calls nearestPortal() on Y-keypress and only sends ClientRequestSender.sendStartExtract(portal.entityId()) when it is non-null — so the player can never trigger extraction via the client-side proximity check. The collapse-race "本族裂口" exit-portal list also stays permanently empty.
- **证据**:client/src/main/java/com/bong/client/network/ExtractServerDataHandler.java:25 (readString(payload,"direction") stored raw into RiftPortalView.direction()); client/src/main/java/com/bong/client/tsy/ExtractStateStore.java:63 (`if (!"exit".equals(portal.direction())) continue;` inside nearestPortal()); client/src/main/java/com/bong/client/hud/ExtractProgressHudPlanner.java:159 (`.filter(portal -> "exit".equals(portal.direction()))`)  |  proto/bong/envelope.proto:2731-2735 (enum RiftPortalDirection), :2740 (direction field, no stripEnumPrefix fixup registered for RIFT_PORTAL_STATE in ProtoServerDataBridge)
- **修法**:Add stripEnumPrefix(root, "direction", "RIFT_PORTAL_DIRECTION_") (and "kind", "RIFT_PORTAL_KIND_") in a dedicated bridgeRiftPortalState fixup, mirroring the existing bridgeDeathScreen pattern.

### [CRITICAL] `search_aborted` · `reason` (B_enum_case)
- **proto 实际输出**:SearchAbortReason enum (envelope.proto:2841-2847) serializes as full-prefixed name string, e.g. "SEARCH_ABORT_REASON_MOVED" / "SEARCH_ABORT_REASON_COMBAT" / "SEARCH_ABORT_REASON_DAMAGED" / "SEARCH_ABORT_REASON_CANCELLED" / "SEARCH_ABORT_REASON_UNSPECIFIED".
- **handler 期望**:SearchHudStateStore.abortReason(String) does a raw switch/case on lowercase, unprefixed literals: "moved", "combat", "damaged", "cancelled".
- **运行时后果**:readString(payload,"reason") always returns the un-stripped full-prefix string (e.g. "SEARCH_ABORT_REASON_MOVED"), which never matches any case in SearchHudStateStore.abortReason() → always falls through to default AbortReason.NONE. The search-abort HUD can never distinguish moved/combat/damaged/cancelled — all abort causes render identically as the generic 'no reason' state.
- **证据**:ContainerInteractionHandler.java:98 (readString(payload, "reason") passed straight into SearchHudStateStore.markAborted); SearchHudStateStore.java:41-49 (private static AbortReason abortReason(String reason) switch statement, default -> AbortReason.NONE)  |  proto/bong/envelope.proto:2841-2847 (enum SearchAbortReason), proto/bong/envelope.proto:2889-2894 (message SearchAborted, field reason = 3)
- **修法**:SEARCH_ABORTED payloadCase is not in ProtoServerDataBridge's specialCased if-list (lines 249-303); grep confirms stripEnumPrefix() is only invoked for movement_action/zone_kind/phase/outcome/channel/priority/death_screen-stage/qi_color/craft-category — never for SearchAborted's "reason". Add a bridgeSearchAborted() fixup calling stripEnumPrefix(root, "reason", "SEARCH_ABORT_REASON_"), or make abortReason() compare against the full proto enum name.

### [CRITICAL] `yidao_hud_state` · `active_skill` (B_enum_case)
- **proto 实际输出**:optional YidaoSkillId active_skill = 5 (proto/bong/envelope.proto:2993) is a real protobuf enum (envelope.proto:2971-2978). When set, JsonFormat.printer().preservingProtoFieldNames() renders it as the full prefixed enum name string, e.g. "YIDAO_SKILL_ID_MERIDIAN_REPAIR" (not the Rust-side snake_case wire name). When unset it is a synthetic-oneof scalar so it is omitted from the JSON entirely (not "").
- **handler 期望**:YidaoServerDataHandler.java:32 calls readString(payload, "active_skill", "") which just returns the raw string as-is (no enum-prefix stripping/lowercasing). Downstream, YidaoHudPlanner.skillLabel() (client/src/main/java/com/bong/client/hud/YidaoHudPlanner.java:81-90) does an exact switch on lowercase snake_case values without prefix: "meridian_repair", "contam_purge", "emergency_resuscitate", "life_extension", "mass_meridian_repair".
- **运行时后果**:Whenever a healer NPC is actually casting/holding a skill, the client-side HUD (医道 skill line, YidaoHudPlanner.identityLine/skillLabel) silently falls into the switch's `default -> "待机"` branch and always displays the medic as idle, hiding the true active skill from the player with no error/log.
- **证据**:client/src/main/java/com/bong/client/yidao/YidaoServerDataHandler.java:32 (readString call); client/src/main/java/com/bong/client/hud/YidaoHudPlanner.java:81-90 (switch consuming the stored value)  |  proto/bong/envelope.proto:2971-2978 (enum YidaoSkillId, prefixed UPPER_SNAKE values) and proto/bong/envelope.proto:2993 (optional YidaoSkillId active_skill = 5); server/src/schema/proto_convert.rs:2776-2785 (yidao_skill_id_to_proto maps Rust enum -> real proto enum, not a string) and :2793 (active_skill: s.active_skill.as_ref().map(yidao_skill_id_to_proto))
- **修法**:Add a fixup in ProtoServerDataBridge for YIDAO_HUD_STATE that maps the enum name to the snake_case wire value expected by skillLabel() (strip "YIDAO_SKILL_ID_" prefix + lowercase), or change YidaoHudPlanner.skillLabel() to accept the prefixed enum name directly.

### [CRITICAL] `skill_xp_gain` · `skill` (B_enum_case)
- **proto 实际输出**:enum field, proto3-JSON canonical output = full prefixed enum name string, e.g. "SKILL_ID_HERBALISM" / "SKILL_ID_COMBAT"
- **handler 期望**:SkillId.fromWire(readString(p,"skill")) does an exact String.equals() against lowercase unprefixed wire ids ("herbalism", "combat", ...); any other string returns null
- **运行时后果**:skill_xp_gain routes through the generic path (ProtoServerDataBridge.java:57 registers typeString, line 341 `case SKILL_XP_GAIN: return envelope.getSkillXpGain();` falls into the shared extractInner+printAndNormalize path with no per-field reshape). SkillId.fromWire never matches the prefixed enum name → returns null on every single skill_xp_gain push → SkillEventHandler.handleXpGain always hits `if (skill == null || amount == null) return noOp(...)`. Every server-driven XP gain event is silently dropped: SkillSetStore/SkillRecentEventStore never update from live pushes regardless of which skill or how much XP was granted.
- **证据**:client/src/main/java/com/bong/client/network/SkillEventHandler.java:58 `SkillId skill = SkillId.fromWire(readString(p, "skill"));` ; client/src/main/java/com/bong/client/skill/SkillId.java:34-40 fromWire loop compares `id.wire.equals(wire)` against enum constants HERBALISM("herbalism")..CULTIVATION("cultivation")  |  proto/bong/envelope.proto:505-508 `SkillId skill = 2;` inside `message SkillXpGain`; proto/bong/common.proto:51-59 `enum SkillId { SKILL_ID_UNSPECIFIED=0; SKILL_ID_HERBALISM=1; ... }`; server/src/schema/proto_convert.rs:2403 `skill: skill_id_to_proto(&s.skill)` confirms server emits the proto enum, not a raw string
- **修法**:Add a specialCased bridge method for SKILL_XP_GAIN (and its siblings SkillLvUp/SkillCapChanged/SkillScrollUsed, which share the same skill_id_to_proto conversion) that calls stripEnumPrefix(root, "skill", "SKILL_ID_") before wrapLegacy, mirroring the existing bridgeMovementState pattern.

### [CRITICAL] `player_state` · `realm` (B_enum_case)
- **proto 实际输出**:enum field, proto3-JSON canonical output = full prefixed enum name string, e.g. "REALM_CONDENSE"
- **handler 期望**:readRequiredString(payload,"realm") stores the raw string opaquely; downstream consumers do case-insensitive equality against unprefixed lowercase names ("awaken", "induce", "condense", "solidify", "spirit", "void") with a default fallback on no-match
- **运行时后果**:PLAYER_STATE is not among the specialCased payloadCases in ProtoServerDataBridge.bridge() (only INVENTORY_SNAPSHOT/MOVEMENT_STATE/CAST_SYNC/EVENT_STREAM_PUSH/DEATH_SCREEN/SKILL_CONFIG_SNAPSHOT/SKILL_SNAPSHOT/CULTIVATION_DETAIL/CRAFT_RECIPE_LIST/quick_slot_config/skill_bar_config/forge_session/oneof-flat get bridged); it falls to the generic extractInner+printAndNormalize path with zero reshape. Because normalizeRealmField (which correctly converts "REALM_CONDENSE" -> "Condense" for cultivation_detail) is never applied here, HudRealmGate.tier() always returns 0 for every realm string it receives ("realm_condense" etc. never match a case label) → HUD realm-gated features (QiDensityRadarHudPlanner.atLeastCondense, ThreatIndicatorHudPlanner.atLeastSpirit/atLeastVoid, BongHudOrchestrator.atLeastCondense) permanently believe the player is Awaken-tier and never unlock, regardless of actual progression. RealmLabel.displayName also falls to its default branch and renders the raw proto constant (e.g. "REALM_CONDENSE") instead of the Chinese realm label in CultivationScreen/InspectScreen/NpcDialogueScreen.
- **证据**:client/src/main/java/com/bong/client/network/PlayerStateHandler.java:18-21 `String realm = readRequiredString(payload, "realm");` — stored unmodified into PlayerStateViewModel; consumed at client/src/main/java/com/bong/client/hud/HudRealmGate.java:25-39 `switch (normalized) { case "awaken" -> 0; ... default -> 0; }` (normalized = realm.trim().toLowerCase()) and client/src/main/java/com/bong/client/util/RealmLabel.java:17-25 `switch (trimmed.toLowerCase()) { case "condense" -> "凝脉"; ... default -> trimmed; }`  |  proto/bong/envelope.proto:423 `Realm realm = 2;` inside `message PlayerState`; proto/bong/common.proto:11-19 `enum Realm { REALM_UNSPECIFIED=0; REALM_AWAKEN=1; ... REALM_VOID=6; }`; server/src/schema/proto_convert.rs:612 `realm: realm_str_to_proto(realm)` confirms server emits the proto enum
- **修法**:Route PLAYER_STATE through a dedicated bridge method (or extend the generic path) that calls the existing normalizeRealmField(root, "realm") helper already used by bridgeCultivationDetail, before wrapping.

### [CRITICAL] `alchemy_outcome_resolved` · `bucket` (B_enum_case)
- **proto 实际输出**:proto3-JSON enum field 'bucket' (AlchemyOutcomeBucket) is printed by JsonFormat as the full enum value name string, e.g. "ALCHEMY_OUTCOME_BUCKET_GOOD"/"ALCHEMY_OUTCOME_BUCKET_PERFECT"/"ALCHEMY_OUTCOME_BUCKET_FLAWED"/"ALCHEMY_OUTCOME_BUCKET_WASTE"/"ALCHEMY_OUTCOME_BUCKET_EXPLODE" (never absent since it's a non-optional scalar with includingDefaultValueFields).
- **handler 期望**:Raw string is stored as-is into AlchemyAttemptHistoryStore.Entry.bucket, and downstream consumers switch/equals against lowercase bare words "perfect"/"good"/"flawed"/"waste"/"explode".
- **运行时后果**:Outcome bucket switch statements never match any case, always fall through to default: HUD toast always shows generic "炼废" label with gray color 0xFFB8B8B8 instead of the true outcome label/color, and the alchemy screen's 试药史 (attempt history) list always uses default white color "§f" and prints the raw prefixed enum string (e.g. "ALCHEMY_OUTCOME_BUCKET_GOOD") instead of a readable bucket name — regardless of the actual alchemy outcome.
- **证据**:client/src/main/java/com/bong/client/network/alchemy/AlchemyOutcomeResolvedHandler.java:16 (String bucket = readString(p, "bucket", "")) feeding AlchemyProgressHudPlanner.java:104-110 and :136-141 (switch on "perfect"/"good"/"flawed"/"explode") and AlchemyScreen.java:589-598 (switch on "perfect"/"good"/"flawed"/"waste"/"explode")  |  proto/bong/envelope.proto:752-759 (enum AlchemyOutcomeBucket ALCHEMY_OUTCOME_BUCKET_*) and :763 (AlchemyOutcomeBucket bucket = 1;); handler has no stripEnumPrefix (this typeString is generic path, specialCased=false)
- **修法**:Add a stripEnumPrefix step in AlchemyOutcomeResolvedHandler (or a dedicated bridge fixup for alchemy_outcome_resolved) that strips "ALCHEMY_OUTCOME_BUCKET_" and lowercases, matching the values already expected by AlchemyProgressHudPlanner/AlchemyScreen.

### [CRITICAL] `forge_session` · `current_step` (B_enum_case)
- **proto 实际输出**:proto3-JSON top-level enum field 'current_step' (ForgeStep) is printed as full enum name string e.g. "FORGE_STEP_TEMPERING"/"FORGE_STEP_INSCRIPTION"/"FORGE_STEP_CONSECRATION"/"FORGE_STEP_BILLET"/"FORGE_STEP_DONE" (always present, non-optional scalar).
- **handler 期望**:ForgeSessionHandler reads current_step raw into ForgeSessionStore.Snapshot.currentStep, and numerous UI components compare it via String.equals against lowercase bare words "tempering"/"inscription"/"consecration".
- **运行时后果**:Every currentStep-gated UI branch in the forge minigame silently no-ops: TemperingTrackComponent never renders the tempering track, TemperingInputHandler always rejects tempering key input, InscriptionPanelComponent/ConsecrationPanelComponent never activate their panels, and ForgeScreen's step routing (lines 72-86) never selects any of the three sub-screens — the forge session UI appears permanently stuck/blank regardless of the server's actual current step.
- **证据**:client/src/main/java/com/bong/client/network/forge/ForgeSessionHandler.java:19 (String step = p.has("current_step") ? p.get("current_step").getAsString() : "done") feeding: ForgeScreen.java:72,76,86,246,250 ("tempering"/"inscription"/"consecration".equals(...)); TemperingInputHandler.java:27 (!"tempering".equals(...)); ConsecrationPanelComponent.java:58,109 (!"consecration".equals(...)); InscriptionPanelComponent.java:66,94 (!"inscription".equals(...)); TemperingTrackComponent.java:44 (!"tempering".equals(...))  |  proto/bong/envelope.proto:895-902 (enum ForgeStep FORGE_STEP_*) and :973 (ForgeStep current_step = 5;); bridgeForgeSession fixup (client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java:550-565) only flattens the nested step_state oneof tag (FORGE_STEP_VARIANTS lowercase "billet"/"tempering"/"inscription"/"consecration" at line 547-548) and never touches the sibling top-level current_step enum field
- **修法**:Extend bridgeForgeSession (or add stripEnumPrefix to current_step specifically) to also lowercase-strip the top-level current_step field to "billet"/"tempering"/"inscription"/"consecration"/"done", consistent with the FORGE_STEP_VARIANTS tag values already produced for step_state.

### [CRITICAL] `alchemy_contamination` · `levels[].color` (B_enum_case)
- **proto 实际输出**:proto3-JSON enum field 'color' inside repeated message AlchemyContaminationLevel is printed as full enum name string, e.g. "COLOR_KIND_MELLOW" / "COLOR_KIND_VIOLENT" (never absent, non-optional scalar).
- **handler 期望**:AlchemyContaminationHandler compares color via String.equals("Mellow") / String.equals("Violent") to pick which of the two exposed slots (mellow/violent) to populate.
- **运行时后果**:The color equality checks never match ("COLOR_KIND_MELLOW" != "Mellow"), so mellowCur/mellowMax/mellowOk and violentCur/violentMax/violentOk are never populated from server data — they stay at their initialized defaults (0/0/true) every time, meaning ContaminationWarningStore always reports zero contamination and ok=true regardless of the actual toxin levels the server pushed, silently hiding real contamination warnings from the player.
- **证据**:client/src/main/java/com/bong/client/network/alchemy/AlchemyContaminationHandler.java:32-37 (String color = readString(lvl, "color", ""); if ("Mellow".equals(color)) {...} else if ("Violent".equals(color)) {...})  |  proto/bong/common.proto:63-75 (enum ColorKind COLOR_KIND_* including COLOR_KIND_MELLOW=3, COLOR_KIND_VIOLENT=9) and proto/bong/envelope.proto:794 (ColorKind color = 1;) inside AlchemyContaminationLevel; alchemy_contamination is generic path, specialCased=false, no enum-stripping fixup
- **修法**:Compare against the proto enum's full names ("COLOR_KIND_MELLOW"/"COLOR_KIND_VIOLENT") or strip the "COLOR_KIND_" prefix and match case-insensitively before branching.

### [CRITICAL] `botany_plant_v2_render_profiles` · `profiles[].model_overlay` (B_enum_case)
- **proto 实际输出**:model_overlay is enum BotanyModelOverlay { BOTANY_MODEL_OVERLAY_UNSPECIFIED=0; BOTANY_MODEL_OVERLAY_NONE=1; BOTANY_MODEL_OVERLAY_EMISSIVE=2; BOTANY_MODEL_OVERLAY_DUAL_PHASE=3; }. proto3-JSON prints the full prefixed enum name string, e.g. "BOTANY_MODEL_OVERLAY_EMISSIVE" / "BOTANY_MODEL_OVERLAY_DUAL_PHASE".
- **handler 期望**:ModelOverlay.fromWireName() lowercases the raw string and switches on bare unprefixed tokens "emissive" / "dual_phase" with no prefix-stripping logic; anything else (including the actual prefixed proto output) falls to default -> NONE.
- **运行时后果**:typeString="botany_plant_v2_render_profiles" is not specialCased. "BOTANY_MODEL_OVERLAY_EMISSIVE".toLowerCase() = "botany_model_overlay_emissive", which never matches case "emissive", so fromWireName always returns NONE for every profile regardless of server value. Emissive/dual-phase plant visual overlays (glow, day/night tint swap via tintAt()) never render client-side; all plants silently fall back to plain NONE overlay even when the server explicitly sets emissive or dual_phase.
- **证据**:client/src/main/java/com/bong/client/botany/BotanyPlantRenderProfile.java:41-49 (fromWireName: `switch (raw.trim().toLowerCase()) { case "emissive" -> EMISSIVE; case "dual_phase" -> DUAL_PHASE; default -> NONE; }`), called from client/src/main/java/com/bong/client/network/BotanyPlantRenderProfileHandler.java:37  |  proto/bong/envelope.proto:1218-1223 (enum BotanyModelOverlay) and proto/bong/envelope.proto:1250 (BotanyModelOverlay model_overlay = 5 in BotanyPlantV2RenderProfile)
- **修法**:Strip the "BOTANY_MODEL_OVERLAY_" prefix before lowercasing/switching in fromWireName (or add stripEnumPrefix handling similar to other bridged enums).

### [CRITICAL] `gathering_session` · `target_type` (B_enum_case)
- **proto 实际输出**:enum GatheringTargetType → proto3-JSON outputs the full prefixed enum name string, e.g. "GATHERING_TARGET_TYPE_WOOD" / "GATHERING_TARGET_TYPE_ORE"
- **handler 期望**:GatheringSessionHandler stores the raw string as-is; GatheringSessionViewModel's compact constructor lowercases it (`targetType.toLowerCase()`) and displayTargetName() does `switch (targetType) { case "ore" ... case "wood" ... default -> "草药" }` expecting the bare unprefixed value
- **运行时后果**:lowercased value is "gathering_target_type_wood"/"gathering_target_type_ore", which never matches the switch's "wood"/"ore" cases → displayTargetName() always falls through to the default "草药" label even when gathering ore or wood, unless the server-supplied target_name string is already non-blank (which masks this bug only when target_name happens to be set).
- **证据**:client/src/main/java/com/bong/client/network/GatheringSessionHandler.java:26 (`readOptionalString(payload, "target_type")`); client/src/main/java/com/bong/client/gathering/GatheringSessionViewModel.java:41 (lowercase), :122-131 (switch on "ore"/"wood")  |  proto/bong/envelope.proto:1303-1308 (GatheringTargetType enum, values GATHERING_TARGET_TYPE_HERB/ORE/WOOD)
- **修法**:add a stripEnumPrefix step (either in a dedicated fixup or in GatheringSessionHandler) that strips "GATHERING_TARGET_TYPE_" and lowercases before storing

### [CRITICAL] `lingtian_session` · `kind` (B_enum_case)
- **proto 实际输出**:enum LingtianSessionKind → proto3-JSON outputs full prefixed name, e.g. "LINGTIAN_SESSION_KIND_PLANTING"
- **handler 期望**:readString(p,"kind","till") returns the raw string; LingtianSessionStore.Kind.fromWire() lowercases and switches on bare values "till"/"renew"/"planting"/"harvest"/"replenish"/"drain_qi", defaulting to TILL for anything unmatched
- **运行时后果**:wire value "lingtian_session_kind_planting" (etc.) never matches fromWire's cases → Kind always resolves to the default TILL("开垦"), so the 灵田 HUD/progress UI always labels every session as 开垦 even during 种植/收获/翻新/补灵/吸灵, and any UI branching on Kind (e.g. icon selection) is permanently wrong.
- **证据**:client/src/main/java/com/bong/client/network/lingtian/LingtianSessionHandler.java:37 (`readString(p, "kind", "till")`), :54 (passed into `LingtianSessionStore.Kind.fromWire(kindStr)`)  |  proto/bong/envelope.proto:1337-1346 (LingtianSessionKind enum)
- **修法**:strip "LINGTIAN_SESSION_KIND_" prefix before lowercasing/matching, either via a dedicated fixup or directly in LingtianSessionHandler/Kind.fromWire

### [CRITICAL] `carrier_state` · `phase` (B_enum_case)
- **proto 实际输出**:enum CarrierChargePhase → proto3 JSON prints the full prefixed enum name, e.g. "CARRIER_CHARGE_PHASE_CHARGING", "CARRIER_CHARGE_PHASE_CHARGED", "CARRIER_CHARGE_PHASE_IDLE"
- **handler 期望**:readPhase() does a raw string switch on bare lowercase literals "charging" / "charged" (no prefix, no stripEnumPrefix), defaulting to IDLE for anything else
- **运行时后果**:CarrierState.phase never matches "charging"/"charged" strings emitted by proto3 JSON → readPhase() always falls into the default branch → client-side carrier phase is permanently reported as IDLE even while the server-side carrier is actively charging or fully charged; charge-phase-dependent HUD/animation never triggers
- **证据**:client/src/main/java/com/bong/client/combat/handler/CarrierStateHandler.java:31-38  |  proto/bong/envelope.proto:1771-1776 (enum CarrierChargePhase), :1781 (phase field)
- **修法**:Strip the "CARRIER_CHARGE_PHASE_" prefix and lowercase before the switch, or switch directly on the full enum-name strings

### [CRITICAL] `false_skin_state` · `layers[].tier` (B_enum_case)
- **proto 实际输出**:FalseSkinLayerState.tier is enum FalseSkinTier → proto3 JSON prints full prefixed name, e.g. "FALSE_SKIN_TIER_LIGHT", "FALSE_SKIN_TIER_MID", "FALSE_SKIN_TIER_HEAVY", "FALSE_SKIN_TIER_ANCIENT", "FALSE_SKIN_TIER_FAN"
- **handler 期望**:FalseSkinStateHandler.readLayers() reads the raw string via readString(layer,"tier","fan") and passes it straight into FalseSkinHudStateStore.Layer, whose compact constructor sanitizeTier() switches on bare lowercase literals "fan"/"light"/"mid"/"heavy"/"ancient" with default → "fan"
- **运行时后果**:Every false-skin armor layer's tier is silently coerced to "fan" (the lowest visual/mechanical tier) regardless of the actual server-assigned tier (light/mid/heavy/ancient) — tier-dependent visuals and any client-side tier-based logic are wrong for all non-fan layers, with no error surfaced
- **证据**:client/src/main/java/com/bong/client/combat/handler/FalseSkinStateHandler.java:69-76 (readLayers, tier read at :70); client/src/main/java/com/bong/client/combat/store/FalseSkinHudStateStore.java:101-107 (sanitizeTier)  |  proto/bong/envelope.proto:1797-1804 (enum FalseSkinTier), :1808 (tier field on FalseSkinLayerState)
- **修法**:Strip "FALSE_SKIN_TIER_" prefix and lowercase before assigning tier, both in the handler's readString call and/or in sanitizeTier()

### [CRITICAL] `qi_color_observed` · `main (及 secondary)` (B_enum_case)
- **proto 实际输出**:proto ColorKind 枚举取值带 `COLOR_KIND_` 前缀全大写全名，如 `COLOR_KIND_SHARP`（common.proto:63-75），generic path（无 fixup）直接原样输出这个字符串
- **handler 期望**:客户端 ColorKind.fromWire(wire) 把 wire 转小写后与 `kind.name().toLowerCase()`（即 "sharp"/"heavy"/... 不带前缀）做 equals 比较
- **运行时后果**:"color_kind_sharp" 永远不等于 "sharp" 等任何客户端枚举名 → main 恒为 null → QiColorObservedHandler.handle() 对每一条 qi_color_observed 消息都返回 noOp；神识观色功能（QiColorObservedStore 从不被写入）在生产环境完全静默失效
- **证据**:client/src/main/java/com/bong/client/network/QiColorObservedHandler.java:13 `ColorKind main = ColorKind.fromWire(readString(payload, "main"));` + :14-19 main==null 时直接 noOp；client/src/main/java/com/bong/client/cultivation/ColorKind.java:34-41 fromWire 用 `kind.name().toLowerCase(Locale.ROOT).equals(normalized)` 比较，无任何前缀剥离  |  proto/bong/common.proto:63-75 `enum ColorKind { COLOR_KIND_UNSPECIFIED=0; COLOR_KIND_SHARP=1; ... COLOR_KIND_TURBID=10; }`
- **修法**:给 qi_color_observed 加专属 bridge fixup 剥离 "COLOR_KIND_" 前缀（可复用 stripEnumPrefix 帮助方法），或客户端 ColorKind.fromWire 兼容带前缀全名

### [CRITICAL] `spiritual_sense_targets` · `entries[].kind` (B_enum_case)
- **proto 实际输出**:SenseKind enum emits the full SCREAMING_SNAKE_CASE name with prefix, e.g. "SENSE_KIND_SPIRIT_EYE", "SENSE_KIND_DYING_ELDER_QI", "SENSE_KIND_LIVING_QI".
- **handler 期望**:SenseKind.fromWire() switches on exact unprefixed PascalCase strings: "SpiritEye", "DyingElderQi", "AmbientLeyline", "CrisisPremonition", etc.; any unmatched string (default branch) silently maps to LIVING_QI.
- **运行时后果**:No fixup exists for SPIRITUAL_SENSE_TARGETS in ProtoServerDataBridge.bridge() (falls straight to generic extractInner path, envelope.proto CASE mapped at ProtoServerDataBridge.java:128, no special branch). Every entry's kind mismatches → always resolves to LIVING_QI. Concretely this means: (a) DyingElderEncounterStore.setSpiritEyeActive(...) in SpiritualSenseTargetsHandler.java:29-31 is always false, so the dying-elder betrayal-probability HUD (plan-dying-elder-v1 M1) never shows real data even when the player has spirit eye active; (b) every realm-vision perception overlay (crisis premonition, zhenfa ward alert, disguised spider/daozhan silhouettes, dying elder qi) renders with the generic living-qi visual instead of its distinct color/behavior — the entire senseKind differentiation feature is dead on arrival.
- **证据**:client/src/main/java/com/bong/client/visual/realm_vision/SpiritualSenseStateReducer.java:33 (SenseKind.fromWire(readString(entry,"kind"))); client/src/main/java/com/bong/client/visual/realm_vision/SenseKind.java:20-41  |  proto/bong/envelope.proto:2492-2510 (enum SenseKind, SENSE_KIND_* prefix), 2512-2517 (SenseEntry.kind = SenseKind)
- **修法**:Add a dedicated bridge fixup for SPIRITUAL_SENSE_TARGETS that strips the SENSE_KIND_ prefix per entry (stripEnumPrefix-style) and/or change SenseKind.fromWire() to accept the SCREAMING_SNAKE_CASE proto names.

### [CRITICAL] `event_alert` · `event` (B_enum_case)
- **proto 实际输出**:EventKind enum emits full prefixed name, e.g. "EVENT_KIND_REALM_COLLAPSE", "EVENT_KIND_THUNDER_TRIBULATION".
- **handler 期望**:parseRealmCollapseHudState compares readOptionalString(payload,"event") (lower-cased/trimmed) against the literal "realm_collapse"; deriveTitleFromEvent() also consumes this same raw string to synthesize a human title by splitting on '_' and title-casing each segment.
- **运行时后果**:EVENT_ALERT has no dedicated bridge fixup (generic extractInner path only, ProtoServerDataBridge.java:413). The literal-equals check against "realm_collapse" can never be true ("EVENT_KIND_REALM_COLLAPSE" != "realm_collapse") → RealmCollapseHudState.empty() is returned unconditionally → the realm-collapse countdown HUD never activates from real server traffic. Additionally, since `title` doesn't exist in the proto (see next finding) the fallback title is always derived from this raw enum string, so toasts show garbage like "Event Kind Realm Collapse：..." with a leaked "Event Kind" prefix instead of a clean event name.
- **证据**:client/src/main/java/com/bong/client/network/EventAlertHandler.java:63-77 (parseRealmCollapseHudState, literal-equals "realm_collapse"), 136-161 (deriveTitleFromEvent)  |  proto/bong/envelope.proto:2565-2579 (enum EventKind, EVENT_KIND_* prefix), 2582-2587 (EventAlert.event = EventKind)
- **修法**:Add a dedicated EVENT_ALERT bridge fixup that strips the EVENT_KIND_ prefix (stripEnumPrefix pattern already used for movement_action/cast phase/etc.), then update the realm_collapse comparison and title derivation to match the stripped SCREAMING_SNAKE_CASE or lower it explicitly.

### [CRITICAL] `skill_lv_up` · `skill` (B_enum_case)
- **proto 实际输出**:`SkillId skill = 2` is a plain (non-oneof, non-optional) enum field on SkillLvUp; proto3-JSON canonical output emits the full enum-value name with prefix, e.g. "SKILL_ID_COMBAT" (or "SKILL_ID_UNSPECIFIED" for default 0).
- **handler 期望**:SkillEventHandler.handleLvUp calls `SkillId.fromWire(readString(p, "skill"))`, and SkillId.fromWire does exact string equality against lowercase wire ids ("herbalism", "combat", etc., no prefix) — never matches an uppercase SKILL_ID_* token.
- **运行时后果**:SkillId.fromWire always returns null for skill_lv_up → `skill == null` short-circuit → ServerDataDispatch.noOp every time. Level-up events for any skill silently never update SkillSetStore/SkillRecentEventStore, i.e. the entire skill_lv_up channel is a dead no-op on the client.
- **证据**:client/src/main/java/com/bong/client/network/SkillEventHandler.java:87-92; client/src/main/java/com/bong/client/skill/SkillId.java:34-40  |  proto/bong/envelope.proto:2899-2903 (message SkillLvUp); proto/bong/common.proto:51-59 (enum SkillId)
- **修法**:Register skill_lv_up as a specialCased payloadCase in ProtoServerDataBridge and stripEnumPrefix(root, "skill", "SKILL_ID_") (lowercased) before generic path, same pattern as bridgeMovementState/bridgeCastSync do for their enum fields.

### [CRITICAL] `skill_cap_changed` · `skill` (B_enum_case)
- **proto 实际输出**:`SkillId skill = 2` on SkillCapChanged, same enum as above → proto3-JSON emits "SKILL_ID_COMBAT" style prefixed full name.
- **handler 期望**:SkillEventHandler.handleCapChanged calls `SkillId.fromWire(readString(p, "skill"))`, requiring exact lowercase wire-id match.
- **运行时后果**:Identical failure mode to skill_lv_up: SkillId.fromWire returns null → noOp every dispatch → skill cap changes (worldview §4 境界软挂钩 unlock) never reach SkillSetStore, UI cap display permanently stale.
- **证据**:client/src/main/java/com/bong/client/network/SkillEventHandler.java:110-115  |  proto/bong/envelope.proto:2905-2909 (message SkillCapChanged); proto/bong/common.proto:51-59
- **修法**:Same as skill_lv_up: add specialCased bridge + stripEnumPrefix on "skill" field for SKILL_CAP_CHANGED payloadCase.

### [CRITICAL] `skill_scroll_used` · `skill` (B_enum_case)
- **proto 实际输出**:`SkillId skill = 3` on SkillScrollUsed, same enum → proto3-JSON emits prefixed full name (e.g. "SKILL_ID_ALCHEMY").
- **handler 期望**:SkillEventHandler.handleScrollUsed calls `SkillId.fromWire(readString(p, "skill"))`, requiring exact lowercase wire-id match.
- **运行时后果**:Identical failure: SkillId.fromWire returns null → `skill == null` → noOp. 残卷顿悟 (scroll consumption XP grant + consumed_scrolls merge + toast/log entry) never applies client-side even though scroll_id/xp_granted/was_duplicate would otherwise parse fine.
- **证据**:client/src/main/java/com/bong/client/network/SkillEventHandler.java:135-141  |  proto/bong/envelope.proto:2911-2917 (message SkillScrollUsed); proto/bong/common.proto:51-59
- **修法**:Same as skill_lv_up: add specialCased bridge + stripEnumPrefix on "skill" field for SKILL_SCROLL_USED payloadCase.

### [WARN] `rift_portal_state` · `kind` (B_enum_case)
- **proto 实际输出**:Enum full-name string, e.g. "RIFT_PORTAL_KIND_COLLAPSE_TEAR" / "RIFT_PORTAL_KIND_MAIN_RIFT" / "RIFT_PORTAL_KIND_DEEP_RIFT".
- **handler 期望**:Lowercase unprefixed literals "collapse_tear"/"main_rift"/"deep_rift" used in switch expressions and String.equals filters.
- **运行时后果**:kindLabel/kindIcon always fall through to their default branch ("撤离点"/"门") instead of showing the actual portal type; and the collapse-family rift-list filter at line 158 never matches any portal, so the "本族裂口" panel never renders during a collapse race even independent of the direction bug.
- **证据**:client/src/main/java/com/bong/client/network/ExtractServerDataHandler.java:24 (readString(payload,"kind")); client/src/main/java/com/bong/client/hud/ExtractProgressHudPlanner.java:158 (`.filter(portal -> "collapse_tear".equals(portal.kind()))`), :245-261 (kindLabel/kindIcon switch on "main_rift"/"deep_rift"/"collapse_tear")  |  proto/bong/envelope.proto:2724-2729 (enum RiftPortalKind), :2739 (kind field)
- **修法**:Same fixup as the direction finding — strip RIFT_PORTAL_KIND_ prefix in a dedicated bridge method for RIFT_PORTAL_STATE.

### [WARN] `extract_started` · `portal_kind` (B_enum_case)
- **proto 实际输出**:Enum full-name string, e.g. "RIFT_PORTAL_KIND_MAIN_RIFT".
- **handler 期望**:Lowercase unprefixed literal compared in ExtractProgressHudPlanner.kindLabel switch.
- **运行时后果**:During an active extraction, the in-progress HUD bar label always shows the generic fallback "撤离点" instead of the correct portal-type label ("主裂缝"/"深层缝"/"塌缩裂口").
- **证据**:client/src/main/java/com/bong/client/network/ExtractServerDataHandler.java:49 (readString(payload,"portal_kind") -> ExtractStateStore.markStarted); client/src/main/java/com/bong/client/tsy/ExtractStateStore.java:79-84 (markStarted stores portalKind as activePortalKind); client/src/main/java/com/bong/client/hud/ExtractProgressHudPlanner.java:73,245-252 (kindLabel(state.activePortalKind()))  |  proto/bong/envelope.proto:2757-2763 (message ExtractStarted, portal_kind field 3); :2724-2729 (enum RiftPortalKind)
- **修法**:Add a bridgeExtractStarted fixup stripping the RIFT_PORTAL_KIND_ prefix from portal_kind.

### [WARN] `extract_aborted` · `reason` (B_enum_case)
- **proto 实际输出**:Enum full-name string, e.g. "EXTRACT_ABORTED_REASON_ALREADY_BUSY" / "EXTRACT_ABORTED_REASON_MOVED" / etc.
- **handler 期望**:Lowercase unprefixed literals ("moved","combat","damaged","out_of_range","not_in_tsy","already_busy","portal_occupied","cannot_exit","cancelled") switched on in reasonLabel() and isRejectionReason().
- **运行时后果**:reasonLabel() never matches any case so it always falls to `default -> "未明"`, showing a generic unhelpful abort reason instead of the real one. isRejectionReason() also always returns false, so markAborted always takes the "撤离中断：" branch and always applies the red screen flash — even for reject-class reasons (already_busy/out_of_range/not_in_tsy/portal_occupied/cannot_exit) where the code intends to suppress the flash and use the "无法撤离：" prefix instead.
- **证据**:client/src/main/java/com/bong/client/network/ExtractServerDataHandler.java:73 (readString(payload,"reason") -> ExtractStateStore.markAborted); client/src/main/java/com/bong/client/tsy/ExtractStateStore.java:100-107 (markAborted), :185-207 (reasonLabel/isRejectionReason switches)  |  proto/bong/envelope.proto:2782-2799 (enum ExtractAbortedReason, message ExtractAborted, reason field)
- **修法**:Add a bridgeExtractAborted fixup stripping EXTRACT_ABORTED_REASON_ prefix from reason.

### [WARN] `extract_failed` · `reason` (B_enum_case)
- **proto 实际输出**:Enum full-name string, e.g. "EXTRACT_FAILED_REASON_SPIRIT_QI_DRAINED".
- **handler 期望**:Lowercase unprefixed literal "spirit_qi_drained" switched on in reasonLabel().
- **运行时后果**:reasonLabel() never matches "spirit_qi_drained" so the failure banner always shows the generic fallback "未明" instead of "真元耗尽".
- **证据**:client/src/main/java/com/bong/client/network/ExtractServerDataHandler.java:77 (readString(payload,"reason") -> ExtractStateStore.markFailed); client/src/main/java/com/bong/client/tsy/ExtractStateStore.java:109-114 (markFailed), :185-200 (reasonLabel switch)  |  proto/bong/envelope.proto:2801-2809 (enum ExtractFailedReason, message ExtractFailed)
- **修法**:Add a bridgeExtractFailed fixup stripping EXTRACT_FAILED_REASON_ prefix from reason.

### [WARN] `container_state` · `kind` (B_enum_case)
- **proto 实际输出**:ContainerKind enum (envelope.proto:2824-2832) serializes as full-prefixed name string, e.g. "CONTAINER_KIND_DRY_CORPSE", "CONTAINER_KIND_SKELETON", "CONTAINER_KIND_STORAGE_POUCH", "CONTAINER_KIND_STONE_CASKET", "CONTAINER_KIND_RELIC_CORE", "CONTAINER_KIND_SURFACE_STASH".
- **handler 期望**:Stored value is later switched on in TsyContainerView.kindLabelZh() against lowercase unprefixed literals: "dry_corpse", "skeleton", "storage_pouch", "stone_casket", "relic_core".
- **运行时后果**:Currently unreachable in practice because the world_pos defect above causes handleContainerState to noOp before any TsyContainerView is ever constructed. However this is an independent second defect: the moment the world_pos bug is fixed, kind will still never match any case in kindLabelZh(), so every container will permanently render the generic '容器' label instead of the differentiated 干尸/骨架/储物袋残骸/石匣/法阵核心 labels.
- **证据**:ContainerInteractionHandler.java:36 (readString(payload, "kind") stored verbatim into TsyContainerView); TsyContainerView.java:25-34 (public String kindLabelZh() switch statement, default -> "容器")  |  proto/bong/envelope.proto:2824-2832 (enum ContainerKind), proto/bong/envelope.proto:2851 (ContainerKind kind = 2)
- **修法**:Same missing-fixup root cause as the world_pos finding — add a dedicated bridgeContainerState() that strips the CONTAINER_KIND_ prefix and lowercases, matching the pattern used elsewhere (stripEnumPrefix helper already exists at ProtoServerDataBridge.java:968).

### [WARN] `inventory_snapshot` · `forge_color` (B_enum_case)
- **proto 实际输出**:optional ColorKind enum on InventoryItemView; when set, proto3-JSON emits the full prefixed enum name string e.g. "COLOR_KIND_SHARP"; when unset the field is omitted entirely (proto3 optional presence)
- **handler 期望**:readOptionalString(itemObject,"forge_color") stores the raw string opaquely into InventoryItem.forgeColor(); downstream tooltip code does exact-string switch against PascalCase Rust-style variant names ("Sharp","Heavy","Mellow",...) with a passthrough default
- **运行时后果**:inventory_snapshot is specialCased, but bridgeInventorySnapshot (ProtoServerDataBridge.java:607-633) only unwraps the HotbarSlot wrapper array — it never walks into placed_items[].item / equipped.*_worn[] / equipped.*_held / hotbar[].item to strip the forge_color enum prefix. For any consecrated/color-forged item, the tooltip line `forge.append(forgeColorLabel(hoveredItem.forgeColor()))` falls to the default branch and renders the raw proto constant (e.g. "COLOR_KIND_SHARP") instead of the intended glyph ("锐") — non-crashing but visibly broken player-facing text.
- **证据**:client/src/main/java/com/bong/client/network/InventorySnapshotHandler.java:318 `String forgeColor = readOptionalString(itemObject, "forge_color");` (passed unmodified into InventoryItem.createFullWithVisualMeta at line 356); consumed at client/src/main/java/com/bong/client/inventory/component/ItemTooltipPanel.java:253-266 `private static String forgeColorLabel(String color) { return switch (color) { case "Sharp" -> "锐"; ... default -> color; }; }`  |  proto/bong/envelope.proto (InventoryItemView, `optional ColorKind forge_color = 18;`) inside message InventoryItemView (fields listed at lines 523-546); proto/bong/common.proto:63-73 `enum ColorKind { COLOR_KIND_UNSPECIFIED=0; COLOR_KIND_SHARP=1; ... }`
- **修法**:Extend bridgeInventorySnapshot to recurse into every InventoryItemView-shaped object (placed_items[].item, equipped.*_worn[]/*_held, hotbar[].item) and stripEnumPrefix(item, "forge_color", "COLOR_KIND_") before wrapLegacy.

### [WARN] `forge_outcome` · `bucket` (B_enum_case)
- **proto 实际输出**:`ForgeOutcomeBucket bucket = 3;` is a plain (non-optional) enum scalar, always printed by includingDefaultValueFields as the full prefixed name string, e.g. "FORGE_OUTCOME_BUCKET_WASTE" / "FORGE_OUTCOME_BUCKET_PERFECT" (proto/bong/envelope.proto:983, enum defined at :913-920).
- **handler 期望**:ForgeOutcomeHandler.java:20 reads `p.get("bucket").getAsString()` directly with no prefix stripping, defaulting only to the bare lowercase literal "waste" when the key is missing; the raw value is stored verbatim into `ForgeOutcomeStore.Snapshot.bucket` and rendered directly in ForgeScreen.java:110 (`"§l上次结果: §r" + outcome.bucket()`).
- **运行时后果**:forge_outcome is NOT in the 14 special-cased bridge methods (only FORGE_OUTCOME appears in the type map and extractInner switch, ProtoServerDataBridge.java:69,353) so it goes through the fully generic printAndNormalize path with no stripEnumPrefix call anywhere for this payload. Result: the forge screen's '上次结果' (last outcome) line always shows the raw enum constant like 'FORGE_OUTCOME_BUCKET_PERFECT' instead of a clean word — a cosmetic but real display corruption on every successful forge, not a functional break (no equality/switch on this field elsewhere in the handler chain that was found).
- **证据**:client/src/main/java/com/bong/client/network/forge/ForgeOutcomeHandler.java:20; client/src/main/java/com/bong/client/forge/ForgeScreen.java:107-114  |  proto/bong/envelope.proto:913-920 (enum ForgeOutcomeBucket), :983 (bucket field, not `optional` — always present in generic JSON)
- **修法**:Add a small bridgeForgeOutcome fixup (or reuse stripEnumPrefix) to strip "FORGE_OUTCOME_BUCKET_" and lowercase, matching what bridgeCraftRecipeList already does for CRAFT_CATEGORY_.

### [WARN] `forge_outcome` · `color` (B_enum_case)
- **proto 实际输出**:`optional ColorKind color = 6;` — when set, proto3-JSON prints the full prefixed enum name, e.g. "COLOR_KIND_SHARP" (proto/bong/envelope.proto:986; enum at proto/bong/common.proto:63-75). When unset, the field is correctly absent (explicit optional desugars to a one-field oneof, so includingDefaultValueFields does not force it to print) — handler's has()/isJsonNull() guard for absence is correct.
- **handler 期望**:ForgeOutcomeHandler.java:24-25 reads `p.get("color").getAsString()` verbatim (only null-guards absence, no prefix stripping) and stores it as `colorName` in ForgeOutcomeStore.Snapshot, displayed directly in ForgeScreen.java:109 (`" 色=" + outcome.colorName()`).
- **运行时后果**:Same generic-path gap as bucket above: when a forged weapon rolls a qi color, the forge screen shows the raw '色=COLOR_KIND_SHARP' instead of the intended color label. Cosmetic display corruption, no logic branch depends on the exact string in the code paths inspected.
- **证据**:client/src/main/java/com/bong/client/network/forge/ForgeOutcomeHandler.java:24-25; client/src/main/java/com/bong/client/forge/ForgeScreen.java:109  |  proto/bong/common.proto:63-75 (enum ColorKind, 10 named colors 锐/厚/醇/实/轻/巧/柔/阴/烈/浊); proto/bong/envelope.proto:986 (color field)
- **修法**:Same fixup as bucket — strip "COLOR_KIND_" prefix and lowercase/map to display name before storing into Snapshot.colorName.

### [WARN] `gathering_session` · `quality_hint` (B_enum_case)
- **proto 实际输出**:enum GatheringQualityHint → proto3-JSON outputs full prefixed name, e.g. "GATHERING_QUALITY_HINT_PERFECT_POSSIBLE"
- **handler 期望**:GatheringSessionHandler stores raw string; ViewModel lowercases and does `switch (qualityHint) { case "perfect","perfect_possible" -> ...; case "fine","fine_likely" -> ...; }` expecting bare unprefixed values; hasPerfectQualityHint() likewise does `"perfect".equals(qualityHint)`
- **运行时后果**:lowercased value like "gathering_quality_hint_perfect_possible" never matches any switch case → qualityLabel() always returns "" and hasPerfectQualityHint() always returns false, so the 极品/优良 quality-hint tag never shows during active gathering regardless of actual server-computed quality tier.
- **证据**:client/src/main/java/com/bong/client/network/GatheringSessionHandler.java:27 (`readOptionalString(payload, "quality_hint")`); client/src/main/java/com/bong/client/gathering/GatheringSessionViewModel.java:42 (lowercase), :133-144 (qualityLabel/hasPerfectQualityHint switches)  |  proto/bong/envelope.proto:1310-1318 (GatheringQualityHint enum)
- **修法**:same prefix-stripping fixup as target_type, applied to quality_hint too

### [WARN] `realm_vision_params` · `fog_shape` (B_enum_case)
- **proto 实际输出**:proto FogShape 枚举取值为 `FOG_SHAPE_UNSPECIFIED/FOG_SHAPE_CYLINDER/FOG_SHAPE_SPHERE`（envelope.proto:2469-2473），generic path 无 fixup，原样输出全名字符串
- **handler 期望**:客户端 FogShape.fromWire(wireName) 的 switch 只精确匹配字面量 "Sphere" 或 "sphere"，其余（含 default）一律返回 CYLINDER
- **运行时后果**:服务端发 "FOG_SHAPE_SPHERE" 时永远不匹配 "Sphere"/"sphere"，落入 default → 客户端雾效形状恒为 CYLINDER，SPHERE 雾形态（通常对应高境界视觉差异化）在生产环境永远不会生效，静默劣化视觉表现但不阻断整条消息（其余字段仍正常写入 RealmVisionState）
- **证据**:client/src/main/java/com/bong/client/visual/realm_vision/RealmVisionStateReducer.java:24 `FogShape.fromWire(readString(payload, "fog_shape"))`；client/src/main/java/com/bong/client/visual/realm_vision/FogShape.java:7-15 `switch (wireName.trim()) { case "Sphere", "sphere" -> SPHERE; default -> CYLINDER; }`  |  proto/bong/envelope.proto:2469-2473 `enum FogShape { FOG_SHAPE_UNSPECIFIED=0; FOG_SHAPE_CYLINDER=1; FOG_SHAPE_SPHERE=2; }`
- **修法**:给 realm_vision_params 加专属 fixup 剥离 "FOG_SHAPE_" 前缀并转小写，或客户端 FogShape.fromWire 改为大小写/前缀不敏感匹配

### [WARN] `spirit_treasure_dialogue` · `tone` (B_enum_case)
- **proto 实际输出**:enum SpiritTreasureDialogueTone (proto/bong/envelope.proto:3065-3072), 字段 SpiritTreasureDialogueData.tone=5 (proto/bong/envelope.proto:3079)。spirit_treasure_dialogue 走 generic path（specialCased=false；ProtoServerDataBridge.java 里 SPIRIT_TREASURE_DIALOGUE 只在 extractInner switch 里被取出内层消息，没有专属 bridge 方法做 stripEnumPrefix）。proto3 JSON 规则下枚举字段输出全名字符串，如 'SPIRIT_TREASURE_DIALOGUE_TONE_COLD'（带前缀，非 Rust serde 的 'cold'）。
- **handler 期望**:SpiritTreasureDialogueHandler.parse() 用 SpiritTreasureJson.readString(dialogue, "tone") 把该字段当普通字符串原样读入，isBlank 校验通过即存入 SpiritTreasureDialogue.tone；随后 JiZhaoJingTabPanel 直接把它拼进玩家可见文本 '[' + tone + ']'
- **运行时后果**:灵宝对话面板（JiZhaoJingTabPanel）括号内直接显示 'SPIRIT_TREASURE_DIALOGUE_TONE_COLD' 而非设计预期的 'cold' 等短词，玩家可见的 UI 文案永久污染（不触发 noOp，payload 仍被接受写入 store，只是字段值本身错误）
- **证据**:client/src/main/java/com/bong/client/spirittreasure/SpiritTreasureDialogueHandler.java:49 (readString 读取) + 58 (仅 isBlank 校验，无前缀剥离/switch)；client/src/main/java/com/bong/client/spirittreasure/JiZhaoJingTabPanel.java:62 (直接渲染)  |  proto/bong/envelope.proto:3065-3079；server/src/schema/spirit_treasure.rs:10-18 (#[serde(rename_all="snake_case")] 确认原始预期值为 'cold'/'curious'/'warning'/'amused'/'silent')；server/src/schema/proto_convert.rs:2848-2856 (枚举变体→proto int 映射，未处理枚举名前缀问题)
- **修法**:在 ProtoServerDataBridge 新增 bridgeSpiritTreasureDialogue（仿 bridgeCastSync）对 'dialogue.tone' 做 stripEnumPrefix('SPIRIT_TREASURE_DIALOGUE_TONE_')；或在 SpiritTreasureDialogueHandler 里剥前缀+转小写

### [INFO] `container_state` · `locked` (B_enum_case)
- **proto 实际输出**:optional KeyKind locked (envelope.proto:2856) — when set, serializes as full-prefixed enum name e.g. "KEY_KIND_STONE_CASKET_KEY"; when unset (explicit-presence optional field), it is entirely absent from the JSON (not emitted even with includingDefaultValueFields, per proto3 optional presence semantics).
- **handler 期望**:readNullableString(payload, "locked") just extracts the raw string as-is with no downstream consumer contract confirmed to require a specific casing/prefix in the code paths inspected.
- **运行时后果**:Same masking as the kind finding (container_state is currently dropped entirely by the world_pos bug, so locked is never stored). Flagged at lower severity only because no confirmed downstream string-literal comparison against 'locked' was found in the files read for this audit; reported for consistency awareness alongside the kind enum defect.
- **证据**:ContainerInteractionHandler.java:41 (readNullableString(payload, "locked") stored verbatim into TsyContainerView.locked())  |  proto/bong/envelope.proto:2856 (optional KeyKind locked = 7)
- **修法**:If any UI/logic later compares TsyContainerView.locked() against an unprefixed literal (e.g. "stone_casket_key"), it will fail the same way as kind/reason — add stripEnumPrefix(root, "locked", "KEY_KIND_") to the same container_state fixup recommended above for consistency.

### [INFO] `craft_outcome` · `failed.reason` (B_enum_case)
- **proto 实际输出**:CraftOutcomeFailed.reason is `CraftFailureReason reason = 4;`, a plain enum scalar — proto3-JSON prints the full prefixed name, e.g. "CRAFT_FAILURE_REASON_PLAYER_CANCELLED" (proto/bong/envelope.proto:1171-1179, enum at :1070-1075).
- **handler 期望**:CraftOutcomeHandler.java:40 reads `readString(payload, "reason")` verbatim and passes it as `reasonWire` into `CraftStore.CraftOutcomeEvent.failed(...)`, stored as `failureReason` (client/src/main/java/com/bong/client/craft/CraftStore.java:165,187).
- **运行时后果**:bridgeOneofFlat (ProtoServerDataBridge.java:259-266,486-507) flattens the CRAFT_OUTCOME oneof into `kind:"completed"|"failed"` correctly (handler's kind-based switch works fine) but does not strip enum prefixes on the flattened variant's own fields, so `reason` arrives as the raw "CRAFT_FAILURE_REASON_..." string. Verified via `grep -rn "failureReason" client/src/main/java/` that this value is stored but never read anywhere else in the client (no UI toast/log branches on it currently) — so presently a dead/cosmetic-only drift, not an active failure, but any future 'why did my craft fail' UI wired to `CraftOutcomeEvent.failureReason()` will show the raw prefixed constant instead of a readable reason.
- **证据**:client/src/main/java/com/bong/client/network/CraftOutcomeHandler.java:38-43; client/src/main/java/com/bong/client/craft/CraftStore.java:159-188  |  proto/bong/envelope.proto:1070-1075 (enum CraftFailureReason), :1171-1179 (CraftOutcomeFailed.reason)
- **修法**:Extend bridgeOneofFlat's craft_outcome case (or add a small post-processing step) to strip "CRAFT_FAILURE_REASON_" and lowercase, matching the stripEnumPrefix pattern already used for craft_recipe_list.category.

### [INFO] `false_skin_state` · `kind` (B_enum_case)
- **proto 实际输出**:optional FalseSkinKind → when set, proto3 JSON prints full prefixed name e.g. "FALSE_SKIN_KIND_ROTTEN_WOOD_ARMOR"; when unset it is omitted (synthetic-oneof/explicit-presence semantics)
- **handler 期望**:FalseSkinHudStateStore.tierForLegacyKind() compares kind against bare literal "rotten_wood_armor" (no prefix) to pick a legacy-fallback tier when the layers[] array is empty
- **运行时后果**:The legacy-fallback path (only exercised when the server sends an empty layers[] array) always synthesizes tier "fan" instead of "mid" for rotten_wood_armor false-skins; currently secondary/lower-impact because the primary layers[].tier path is separately broken (see the layers[].tier finding above) and masks this one whenever layers[] is populated
- **证据**:client/src/main/java/com/bong/client/combat/store/FalseSkinHudStateStore.java:97-99 (tierForLegacyKind), invoked from :88-89 inside normalizeLayers()  |  proto/bong/envelope.proto:1790-1794 (enum FalseSkinKind), :1818 (optional kind field on FalseSkinState)
- **修法**:Strip "FALSE_SKIN_KIND_" prefix and lowercase before the comparison

## RC3 坐标被拍平 (world_pos_x/y/z / pos_x/y/z) vs handler 读 world_pos 数组  · 11 条

### [CRITICAL] `trade_offer` · `requested_items[].instance_id` (A_shape)
- **proto 实际输出**:TradeItemSummary.instance_id 为 uint64 → JSON 字符串（repeated message 数组内每个元素同样受影响）
- **handler 期望**:同上 parseTradeItem 逻辑，对每个 requested_items 元素调用
- **运行时后果**:即使前两个 trade_offer 缺陷被修好，requested_items 里每一项也都会因 instance_id 恒为 null 而被静默跳过，requestedItems 列表永远为空（交易请求方看不到对方想要什么）
- **证据**:client/src/main/java/com/bong/client/network/SocialServerDataHandler.java:256-260 for (JsonElement element : requestedArray) { ... parseTradeItem(element); if (item != null) requestedItems.add(item); }  |  proto/bong/envelope.proto:2156 repeated TradeItemSummary requested_items = 5;
- **修法**:同上 readLong 修复

### [CRITICAL] `rift_portal_state` · `world_pos` (A_shape)
- **proto 实际输出**:No `world_pos` field exists in proto RiftPortalState at all — the server's internal [f64;3] is flattened into three independent scalar doubles: world_pos_x/world_pos_y/world_pos_z.
- **handler 期望**:A JSON array [x,y,z] under the key "world_pos", read via readDoubleTriple(payload, "world_pos") which requires a JsonArray of size 3.
- **运行时后果**:readDoubleTriple always returns null since the "world_pos" key is never present in the generic-path JSON. That makes `pos == null` always true, so the handler ALWAYS returns noOp for rift_portal_state — ExtractStateStore.upsertPortal() is never called. Rift portals never register client-side at all, permanently breaking the entire TSY extraction feature (nearest-portal detection, Y-key extract trigger, collapse-race HUD list) regardless of any other bug.
- **证据**:client/src/main/java/com/bong/client/network/ExtractServerDataHandler.java:18-21 (`double[] pos = readDoubleTriple(payload, "world_pos"); if (entityId == null || pos == null) return noOp(...)`)  |  proto/bong/envelope.proto:2737-2748 (message RiftPortalState — world_pos_x/y/z fields 5-7, comment "Rust [f64;3] 拆三字段"); server/src/schema/proto_convert.rs:970-982 (world_pos_x: s.world_pos[0], world_pos_y: s.world_pos[1], world_pos_z: s.world_pos[2])
- **修法**:Add a bridgeRiftPortalState fixup in ProtoServerDataBridge that reassembles world_pos_x/y/z into a world_pos array (mirroring how other flattened-coordinate payloads are handled elsewhere), or change the handler to read world_pos_x/world_pos_y/world_pos_z scalars directly.

### [CRITICAL] `container_state` · `world_pos_x / world_pos_y / world_pos_z (read as "world_pos")` (C_field_name)
- **proto 实际输出**:proto has NO world_pos field at all — position is flattened into 3 scalar double fields world_pos_x/world_pos_y/world_pos_z (envelope.proto:2853-2855, comment '// Rust [f64;3] 拆三字段'). Generic JsonFormat output therefore emits {"world_pos_x":..,"world_pos_y":..,"world_pos_z":..}, never a "world_pos" key.
- **handler 期望**:readDoubleTriple(payload, "world_pos") requires payload.get("world_pos") to exist and be a JsonArray of 3 numeric elements.
- **运行时后果**:pos is always null, so handleContainerState ALWAYS returns noOp("Ignoring container_state: missing entity_id/world_pos") — every container_state broadcast from the server is silently dropped. TsyContainerStateStore never receives any entries, so container tracking/interactable-detection/loot-search UI for dry corpses, skeletons, storage pouches, stone caskets and relic cores never activates on the client, and kindLabel(entityId) lookups used by all four search_* handlers permanently fall back to the generic "容器" label since no TsyContainerView is ever stored.
- **证据**:client/src/main/java/com/bong/client/network/ContainerInteractionHandler.java:30 (double[] pos = readDoubleTriple(payload, "world_pos")); implementation at lines 140-158 does element.isJsonArray() check and returns null otherwise; lines 31-33 (if (entityId == null || pos == null) return noOp(...))  |  proto/bong/envelope.proto:2849-2859 (message ContainerStateProto)
- **修法**:CONTAINER_STATE payloadCase is not in ProtoServerDataBridge's specialCased if-list (lines 249-303) and grep for "world_pos" in ProtoServerDataBridge.java returns zero hits — no reshape exists anywhere. Add a bridgeContainerState() fixup that reads world_pos_x/y/z and synthesizes a "world_pos":[x,y,z] array, OR change the handler to read the three flat fields directly.

### [CRITICAL] `alchemy_furnace` · `pos` (C_field_name)
- **proto 实际输出**:AlchemyFurnace proto has no `pos` field at all — position is flattened into three separate optional int32 scalars `pos_x`/`pos_y`/`pos_z` (proto/bong/envelope.proto:704-706). Generic JsonFormat (preservingProtoFieldNames, no fixup) will emit `pos_x`/`pos_y`/`pos_z` as JSON numbers (only present when the Option is Some, per explicit-presence rule for `optional` scalars) — it will NEVER emit a `pos` array field.
- **handler 期望**:AlchemyFurnaceHandler.handle() (client/src/main/java/com/bong/client/network/alchemy/AlchemyFurnaceHandler.java:25-30) does `if (p.has("pos") && p.get("pos").isJsonArray()) { JsonArray arr = p.getAsJsonArray("pos"); if (arr.size()==3) pos = new BlockPos(arr.get(0)...,arr.get(1)...,arr.get(2)...); }` — expects a 3-element JSON array under key `pos`.
- **运行时后果**:`p.has("pos")` is always false ⇒ `pos` stays null in every AlchemyFurnaceStore.Snapshot, regardless of the real furnace position the server sent. AlchemyFurnaceInteractionRules.shouldOpenAlchemyFurnace() (client/src/main/java/com/bong/client/alchemy/AlchemyFurnaceInteractionRules.java:11) does `if (... snapshot.pos() == null) return false;`, and this gate is the sole guard used by MixinClientPlayerInteractionManagerAlchemy.java:162 before opening the alchemy furnace GUI. Result: right-clicking any placed alchemy furnace can never open the AlchemyScreen — the entire client-side furnace-open interaction is permanently dead (silent, no exception, no log).
- **证据**:client/src/main/java/com/bong/client/network/alchemy/AlchemyFurnaceHandler.java:24-30  |  proto/bong/envelope.proto:703-712 (message AlchemyFurnace); server/src/schema/proto_convert.rs:1737-1748 (alchemy_furnace_to_proto flattens d.pos: Option<(i32,i32,i32)> into pos_x/pos_y/pos_z)
- **修法**:Either add an alchemy_furnace-specific bridge fixup in ProtoServerDataBridge that reassembles pos_x/pos_y/pos_z into a `pos` array (matching the pattern used for other flattened-coordinate payloads), or change AlchemyFurnaceHandler to read `pos_x`/`pos_y`/`pos_z` scalars directly (treating absence of any one as null-pos, mirroring proto_convert's Option<(i32,i32,i32)> semantics).

### [CRITICAL] `botany_harvest_progress` · `target_pos` (C_field_name)
- **proto 实际输出**:The Rust Option<[f64;3]> is split into three independent optional scalar fields: `optional double target_pos_x = 13; optional double target_pos_y = 14; optional double target_pos_z = 15;`. There is no field literally named "target_pos" in the proto at all, and even if there were, individual optional scalars never serialize as a JSON array.
- **handler 期望**:readOptionalDoubleTriple(payload, "target_pos") expects a JSON array of exactly 3 numeric elements under key "target_pos".
- **运行时后果**:typeString="botany_harvest_progress" is not specialCased, so generic path applies unmodified. payload.get("target_pos") is always null (the key never exists), so readOptionalDoubleTriple always returns null and HarvestSessionViewModel.targetPos is always null regardless of server data. hasTargetPos() (HarvestSessionViewModel.java:165) is always false, so any UI/marker/hazard-position feature keyed on target_pos never activates for any harvest session, silently.
- **证据**:client/src/main/java/com/bong/client/network/BotanyHarvestProgressHandler.java:39 (readOptionalDoubleTriple(payload, "target_pos")) and its implementation at lines 82-100 (element.isJsonArray() check, array.size()!=3 check)  |  proto/bong/envelope.proto:1239-1241 (optional double target_pos_x/target_pos_y/target_pos_z = 13/14/15, comment '// Rust Option<[f64;3]> 拆三字段')
- **修法**:Read target_pos_x/target_pos_y/target_pos_z as three separate optional doubles and assemble the double[3] client-side (matching how MiningProgress/LumberProgress readers should read ore_pos_x/y/z), or add a fixup that reassembles the flattened fields into a target_pos array before the handler runs.

### [CRITICAL] `lingtian_session` · `pos` (C_field_name)
- **proto 实际输出**:no field named `pos` exists on the wire at all; server flattens the internal [i32;3] into three sibling scalar fields `pos_x`, `pos_y`, `pos_z` (int32, native JSON numbers)
- **handler 期望**:readIntArray3(p, "pos") looks up a JSON array under key "pos"
- **运行时后果**:`obj.has("pos")` is always false → readIntArray3 always returns [0,0,0] regardless of the real tile coordinates sent by the server. LingtianSessionStore.Snapshot.x/y/z are always (0,0,0), so any UI/marker that positions the 灵田 progress indicator at the actual tile is silently wrong (always points at world origin) while elapsed/target ticks still update correctly.
- **证据**:client/src/main/java/com/bong/client/network/lingtian/LingtianSessionHandler.java:38 (`int[] pos = readIntArray3(p, "pos");`), :91-102 (readIntArray3: `if (!obj.has(key) || !obj.get(key).isJsonArray()) return out;` where out defaults to {0,0,0})  |  proto/bong/envelope.proto:1352-1354 (`int32 pos_x = 3; int32 pos_y = 4; int32 pos_z = 5;`); server/src/schema/proto_convert.rs:2306-2308 (`pos_x: s.pos[0], pos_y: s.pos[1], pos_z: s.pos[2],`)
- **修法**:either add a bridgeLingtianSession fixup that assembles pos=[pos_x,pos_y,pos_z] before generic printing, or change readIntArray3 to read pos_x/pos_y/pos_z scalars directly

### [CRITICAL] `breakthrough_cinematic` · `world_pos` (C_field_name)
- **proto 实际输出**:proto BreakthroughCinematic 把坐标拍平成三个独立 double 标量字段 world_pos_x/world_pos_y/world_pos_z（id 9/10/11），generic JSON 里根本没有名为 "world_pos" 的键（更没有数组）
- **handler 期望**:readVec3(payload, "world_pos") 要求存在一个 JsonArray 且 size()==3 的字段 "world_pos"
- **运行时后果**:BreakthroughCinematicPayload.parse() 永远返回 null（worldPos 恒为 null）→ BreakthroughCinematicHandler.handle() 对每一条 breakthrough_cinematic 消息都走 noOp 分支——突破演出（粒子/音效/动作/toast/billboard）全链路永不触发，且无异常日志
- **证据**:client/src/main/java/com/bong/client/cultivation/BreakthroughCinematicPayload.java:59 `double[] worldPos = readVec3(payload, "world_pos");` + :196-210 readVec3 实现（element==null||!isJsonArray→return null）+ :71 `worldPos == null` 触发 parse() 整体返回 null  |  proto/bong/envelope.proto:2341-2343 `double world_pos_x=9; double world_pos_y=10; double world_pos_z=11; // Rust [f64;3] 拆三字段`
- **修法**:generic path 无 fixup；需给 breakthrough_cinematic 加专属 bridge fixup 把 world_pos_x/y/z 合成 world_pos 数组，或改客户端 parse 直接读三个标量字段

### [CRITICAL] `inventory_event` · `from / to` (A_shape)
- **proto 实际输出**:InventoryLocation is a bare `oneof location { container | equip | hotbar }` with NO discriminator field. proto3 canonical JSON for a set oneof member nests it under its own field name, e.g. `{"container":{"container_id":..,"row":..,"col":..}}` or `{"equip":{"slot":"EQUIP_SLOT_HEAD","state":"EQUIP_STATE_WORN"}}` or `{"hotbar":{"index":..}}`. There is no `kind` key anywhere in the object.
- **handler 期望**:InventoryEventHandler.parseLocation() reads a flat tagged object: `readRequiredString(obj, "kind")` first, then switches on "container"/"equip"/"hotbar" and reads sibling scalar fields (container_id/row/col, slot, index) directly off the same object.
- **运行时后果**:readRequiredString(obj,"kind") is always null on the real payload → parseLocation() always returns null → EVERY server-pushed inventory_event of kind "moved" or "dropped" is unconditionally dropped as noOp ("invalid from/to location"). Item drag/move reconciliation from server, armor-equip visual flash, and world-drop sync (DroppedItemStore) never fire from real traffic — only local optimistic client state survives, silently diverging from server truth.
- **证据**:client/src/main/java/com/bong/client/network/InventoryEventHandler.java:493-521 (parseLocation reads obj.get("kind")), used at lines 86-90 ("moved") and 104 ("dropped")  |  proto/bong/envelope.proto:2656-2662 (InventoryLocation oneof, no discriminator field); ProtoServerDataBridge.bridgeOneofFlat (client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java:486-507) only flattens the OUTER InventoryEvent oneof (moved/dropped/stack_changed/durability_changed → "kind") into the envelope; it applies no reshape whatsoever to the nested `from`/`to` InventoryLocation oneof.
- **修法**:Add a dedicated reshape (mirroring stripEnumPrefix/bridgeMovementState style) that converts the nested InventoryLocation object into the flat {"kind":...} shape the handler expects, and strip the EQUIP_SLOT_ prefix from the equip variant's `slot` enum.

### [CRITICAL] `inventory_event` · `world_pos` (C_field_name)
- **proto 实际输出**:No `world_pos` field exists. Coordinates are pre-flattened server-side into three separate scalar fields `world_pos_x`, `world_pos_y`, `world_pos_z` (doubles).
- **handler 期望**:readRequiredArray(payload, "world_pos") expects a 3-element JSON array to build WorldPos(x,y,z).
- **运行时后果**:readRequiredArray always returns null (field absent) → parseWorldPos(null) → null → every "dropped" inventory_event is rejected regardless of the from/to defect above. This is the exact flat-scalar-vs-array bug class this audit was launched to catch.
- **证据**:client/src/main/java/com/bong/client/network/InventoryEventHandler.java:105 (readRequiredArray(payload,"world_pos")), 478-489 (parseWorldPos), 575-581 (readRequiredArray impl)  |  proto/bong/envelope.proto:2676-2678 (world_pos_x/y/z flat scalars, comment: "Rust [f64;3] 拆三字段")
- **修法**:Read world_pos_x/world_pos_y/world_pos_z scalars directly instead of a world_pos array, same pattern needed for DroppedLootEntry (envelope.proto:2711-2713) which has the identical flattening.

### [CRITICAL] `dropped_loot_sync` · `drops[].world_pos` (A_shape)
- **proto 实际输出**:DroppedLootEntry has no world_pos array field; server_side coord is flattened into three scalar doubles world_pos_x / world_pos_y / world_pos_z (proto/bong/envelope.proto:2711-2713), confirmed by server/src/schema/proto_convert.rs:723-725 (`world_pos_x: d.world_pos[0]` etc). proto3-JSON output for each drop entry therefore contains keys `world_pos_x`, `world_pos_y`, `world_pos_z` as JSON numbers, never a `world_pos` array.
- **handler 期望**:DroppedLootSyncHandler.parseEntry reads `JsonArray pos = readRequiredArray(object, "world_pos")` and requires pos.size()==3, then indexes pos.get(0/1/2).
- **运行时后果**:readRequiredArray always returns null (field absent) → pos==null → parseEntry returns null → handler immediately returns noOp for the WHOLE dropped_loot_sync payload (loop aborts on first null entry, DroppedItemStore.replaceAll never called). Ground-loot markers/UI never sync from server; dropped_loot_sync is permanently a no-op regardless of actual drop data.
- **证据**:client/src/main/java/com/bong/client/network/DroppedLootSyncHandler.java:41-49  |  proto/bong/envelope.proto:2706-2719 (message DroppedLootEntry / DroppedLootSync); server/src/schema/proto_convert.rs:718-725
- **修法**:Add a specialCased bridge fixup (bridgeDroppedLootSync) that repacks world_pos_x/y/z into a world_pos array per entry (mirroring how other flattened-coord payloads are handled), or change the handler to read world_pos_x/world_pos_y/world_pos_z as three scalar doubles.

### [WARN] `mining_progress` · `display_name / mineral_id` (C_field_name)
- **proto 实际输出**:message MiningProgress only has: session_id (string), ore_pos_x/ore_pos_y/ore_pos_z (int32), progress (double), interrupted (bool), completed (bool). There is no display_name or mineral_id field anywhere in the message.
- **handler 期望**:MiningProgressHandler passes targetNameFields = {"display_name", "mineral_id"} to GatheringProgressPayloadReader.firstNonBlank, which tries payload.get("display_name") then payload.get("mineral_id") before falling back to the literal default "矿脉".
- **运行时后果**:typeString="mining_progress" is not specialCased. Since "display_name" and "mineral_id" never exist in the proto3-JSON output, firstNonBlank always falls through to the hardcoded fallback "矿脉" for every mining session. The gathering HUD/session UI always shows the generic label "矿脉" instead of the actual ore name (progress bar itself still works since session_id/progress/interrupted/completed are all plain scalars read correctly). ore_pos_x/y/z are also never consumed by this handler.
- **证据**:client/src/main/java/com/bong/client/network/MiningProgressHandler.java:6-13 (GatheringProgressPayloadReader.apply(envelope, "ore", "矿脉", "display_name", "mineral_id")) and client/src/main/java/com/bong/client/network/GatheringProgressPayloadReader.java:49,62-70 (firstNonBlank iterates targetNameFields and falls back to `fallback` if none match)  |  proto/bong/envelope.proto:1280-1288 (message MiningProgress { string session_id=1; int32 ore_pos_x=2; int32 ore_pos_y=3; int32 ore_pos_z=4; double progress=5; bool interrupted=6; bool completed=7; })
- **修法**:MiningProgress carries no name field at all — either add one server-side (e.g. ore_display_name) and wire it through proto_convert.rs, or resolve the display name client-side from ore_pos_x/y/z + a block registry lookup instead of expecting a name field that was never sent.

## RC1 uint64→JSON字符串, readLong 的 isNumber() 门恒 null  · 14 条

### [CRITICAL] `social_pact` · `tick` (A_shape)
- **proto 实际输出**:uint64 → JSON 字符串
- **handler 期望**:readLong() 要求 isNumber()==true
- **运行时后果**:readLong 对字符串编码的 uint64 恒返回 null → tick 永远 null → social_pact 恒走 noOp，盟约建立/解除事件（SocialStateStore.recordRelationship + HUD 提示）永不触发
- **证据**:client/src/main/java/com/bong/client/network/SocialServerDataHandler.java:94 Long tick = readLong(p, "tick"); 96 行 tick == null 触发 noOp  |  proto/bong/envelope.proto:2056 uint64 tick = 4;
- **修法**:同 social_exposure：修 readLong 兼容 isString() 情形

### [CRITICAL] `sparring_invite` · `expires_at_ms` (A_shape)
- **proto 实际输出**:uint64 → JSON 字符串
- **handler 期望**:readLong() 要求 isNumber()==true
- **运行时后果**:expiresAtMs 永远 null → sparring_invite 恒走 noOp，切磋邀请永不写入 SocialStateStore.replaceSparringInvite，也永不弹出邀请提示
- **证据**:client/src/main/java/com/bong/client/network/SocialServerDataHandler.java:186 Long expiresAtMs = readLong(p, "expires_at_ms"); 187 行 expiresAtMs==null 触发 noOp  |  proto/bong/envelope.proto:2138 uint64 expires_at_ms = 7;
- **修法**:同上

### [CRITICAL] `trade_offer` · `expires_at_ms` (A_shape)
- **proto 实际输出**:uint64 → JSON 字符串
- **handler 期望**:readLong() 要求 isNumber()==true
- **运行时后果**:expiresAtMs 永远 null → trade_offer 恒走 noOp（与下面 offered_item.instance_id 缺陷各自独立即可单独致命）
- **证据**:client/src/main/java/com/bong/client/network/SocialServerDataHandler.java:249 Long expiresAtMs = readLong(p, "expires_at_ms"); 252 行 expiresAtMs==null 触发 noOp  |  proto/bong/envelope.proto:2157 uint64 expires_at_ms = 6;
- **修法**:同上

### [CRITICAL] `trade_offer` · `offered_item.instance_id` (A_shape)
- **proto 实际输出**:TradeItemSummary.instance_id 为 uint64 → JSON 字符串
- **handler 期望**:parseTradeItem 内 readLong(object,"instance_id") 要求 isNumber()
- **运行时后果**:offered_item 恒被 parseTradeItem 判定为 null（instance_id 永远解析失败）→ 252 行 offeredItem==null 再次独立触发 noOp，即使 expires_at_ms 缺陷被修好，交易报价依旧 100% 无法生效
- **证据**:client/src/main/java/com/bong/client/network/SocialServerDataHandler.java:250 parseTradeItem(p.get("offered_item")); 314 行 Long instanceId = readLong(object, "instance_id"); 318 行 instanceId==null 时 parseTradeItem 直接 return null  |  proto/bong/envelope.proto:2145 uint64 instance_id = 1;（TradeItemSummary），2155 TradeItemSummary offered_item = 4;（TradeOffer）
- **修法**:同上；readLong 修好后此路径自然恢复

### [CRITICAL] `poison_overdose_event` · `player_entity_id` (A_shape)
- **proto 实际输出**:proto field is `uint64 player_entity_id = 1` (proto/bong/envelope.proto:1753, message PoisonOverdoseEvent). Serialized as JSON string per proto3 canonical JSON rules for 64-bit integer types.
- **handler 期望**:readNonNegativeLong(payload, "player_entity_id") at PoisonTraitServerDataHandler.java:100 requires `primitive.isNumber()` (impl at :124-132); always false for the quoted-string uint64, so always null.
- **运行时后果**:handleOverdose() always returns noOp via the line 102 gate, even though lifespan_penalty_years (plain float) parses fine. The lifespan-warning HUD trigger (lifespanWarningUntilMillis / lifespanYearsLost update at lines 109-117) never fires — players never see the overdose lifespan-loss warning, a critical stakes-communication failure with zero error/log signal.
- **证据**:client/src/main/java/com/bong/client/network/PoisonTraitServerDataHandler.java:100 (playerEntityId read), :102-107 (null-check gates the whole handler), :124-132 (readNonNegativeLong impl)  |  proto/bong/envelope.proto:1753 (uint64 player_entity_id in PoisonOverdoseEvent)
- **修法**:Same root cause and fix as the other two poison_* payloads — this is a single shared-code defect (readNonNegativeLong) manifesting identically across all three message types.

### [CRITICAL] `loot_container_update` · `session_id` (A_shape)
- **proto 实际输出**:uint64 -> JSON string ("session_id": "123")
- **handler 期望**:same readLong()/isNumber() gate as the open case
- **运行时后果**:Same root cause as loot_container_open: sessionId parses to null, handleUpdate returns noOp at line 57. Loot container contents never refresh client-side after the initial open (which itself never succeeds either), so any subsequent item placement/removal on the server is invisible to the player.
- **证据**:client/src/main/java/com/bong/client/network/LootContainerHandler.java:55 (readLong(payload, "session_id")) reusing the same readLong at :156-163  |  proto/bong/envelope.proto:3783 (`uint64 session_id = 1;` in LootContainerUpdate)
- **修法**:Same fix as loot_container_open's readLong.

### [CRITICAL] `defense_window` · `started_at_ms / expires_at_ms` (A_shape)
- **proto 实际输出**:uint64 fields → proto3-JSON canonical output is a JSON STRING (e.g. "started_at_ms":"1719999999999"), not a JSON number
- **handler 期望**:readLong() requires JsonPrimitive.isNumber()==true; for a quoted string Gson parses it as a String primitive, so isNumber() returns false and readLong returns null for both fields
- **运行时后果**:startedAtMs and expiresAtMs are always null → the required-field guard at line 21-22 always trips → handle() always returns ServerDataDispatch.noOp. defense_window NEVER applies to DefenseWindowStore, so the 截脉弹反窗口 HUD red ring (JiemaiRingHudPlanner) never renders, silently, every single time the server pushes this payload.
- **证据**:client/src/main/java/com/bong/client/network/DefenseWindowHandler.java:18-27 (durationMs/startedAtMs/expiresAtMs read + null-guard), :39-46 (readLong: `if (!primitive.isNumber()) return null;`)  |  proto/bong/envelope.proto:1447-1451 (`uint64 started_at_ms = 2; uint64 expires_at_ms = 3;`)
- **修法**:either add a bridgeDefenseWindow fixup that re-serializes started_at_ms/expires_at_ms as JSON numbers, or change DefenseWindowHandler.readLong to also accept isString() primitives and Long.parseLong them (matches JsonFormat's int64-as-string convention)

### [CRITICAL] `death_screen` · `cinematic.phase_tick / cinematic.phase_duration_ticks / cinematic.total_elapsed_ticks / cinematic.total_duration_ticks / cinematic.rebirth_weakened_ticks` (A_shape)
- **proto 实际输出**:DeathCinematicData 里这五个字段都是 proto uint64（id 4/5/6/7/14），JsonFormat 一律输出为 JSON 字符串
- **handler 期望**:DeathCinematicPayloadParser.readLong()/readDurationTicks() 同样要求 `primitive.isNumber()` 为 true，否则回退到 fallback（0L 或 1L）
- **运行时后果**:bridgeDeathScreen 的 fixup 只处理了 cinematic 内的三个枚举字段（phase/zone_kind/roll.result），没有转换这五个 uint64 计时字段；DeathCinematicState 里的 phaseTick/phaseDurationTicks/totalElapsedTicks/totalDurationTicks 恒为 0，phaseDurationTicks/totalDurationTicks 恒被 readDurationTicks 钳到最小值 1，rebirthWeakenedTicks 恒为 0——死亡电影演出的阶段进度/时长计算全部失真，无法反映服务端真实节奏
- **证据**:client/src/main/java/com/bong/client/death/DeathCinematicPayloadParser.java:40-43,50 调用 readLong/readDurationTicks + :85-94 readLong 实现（`if (!primitive.isNumber()) return fallback;`）  |  proto/bong/envelope.proto:2413-2416 `uint64 phase_tick=4; uint64 phase_duration_ticks=5; uint64 total_elapsed_ticks=6; uint64 total_duration_ticks=7;` 与 :2423 `uint64 rebirth_weakened_ticks=14;`
- **修法**:扩展 bridgeDeathScreen 的 cinematic 子对象 fixup，把这五个 uint64 字符串字段转回数值；或 DeathCinematicPayloadParser.readLong 兼容字符串数值

### [WARN] `social_renown_delta` · `tags_added[].last_seen_tick` (A_shape)
- **proto 实际输出**:RenownTag.last_seen_tick 为 uint64 → JSON 字符串
- **handler 期望**:parseRenownTags 内部同样用 readLong(tagObject,"last_seen_tick") 要求 isNumber()
- **运行时后果**:即使顶层 tick 缺陷被修好，tags_added 里每条 tag 也会因 last_seen_tick 恒为 null 而被逐条丢弃，tags 列表永远为空
- **证据**:client/src/main/java/com/bong/client/network/SocialServerDataHandler.java:303 Long lastSeenTick = readLong(tagObject, "last_seen_tick"); 305 行 lastSeenTick==null 时整条 tag 被 continue 跳过  |  proto/bong/envelope.proto:379 uint64 last_seen_tick = 3;（RenownTag message）
- **修法**:同 readLong 修复

### [WARN] `niche_intrusion` · `items_taken` (A_shape)
- **proto 实际输出**:repeated uint64 → JSON 字符串数组，如 ["1001","1002"]（repeated 标量的每个元素按该标量类型的 JSON 表示编码，64位整型仍是字符串）
- **handler 期望**:readLongArray 遍历数组元素时逐个 `primitive.isNumber()` 判断，非 number 直接 continue 跳过
- **运行时后果**:readArray(p,"items_taken") 存在性检查会通过（不阻断 dispatch，niche_intrusion 仍被 handled），但 readLongArray 内每个字符串元素都被 isNumber() 判假过滤掉，最终 NicheGuardianStore 记录的 itemsTaken 永远是空列表，龛侵提示里的物品数量恒显示 0，即便服务端确实报了被盗物品
- **证据**:client/src/main/java/com/bong/client/network/SocialServerDataHandler.java:218 NicheIntrusionAlertHandler.recordIntrusion(intruderId, readLongArray(itemsTakenArray), taintDelta); readLongArray 定义于 334-344，第339行 `if (!primitive.isNumber()) continue;`  |  proto/bong/envelope.proto:2108 repeated uint64 items_taken = 5;
- **修法**:readLongArray 元素解析同样要兼容 isString() 情形

### [WARN] `full_power_exhausted_state` · `started_tick / recovery_at_tick` (A_shape)
- **proto 实际输出**:Both uint64 (envelope.proto:2963-2964, fields 3 and 4) — same proto3-JSON string encoding as above.
- **handler 期望**:readLong() requires p.isNumber(), false for the JSON-string encoding of these two fields.
- **运行时后果**:handleExhausted() would always no-op if this payload were ever emitted, identical failure mode to the two findings above. Currently dormant: server/src/network/full_power_emit.rs:155-159 explicitly documents the server no longer sends this payload type ("不再发 FullPowerExhaustedState S2C payload，旧独立 HUD 路径已废"), and the only construction of ServerDataPayloadV1::FullPowerExhaustedState in that file is inside a test asserting it is NOT emitted (line 598). So this is a latent defect with no live runtime consequence today, but would resurface if the exhausted-HUD path is revived.
- **证据**:client/src/main/java/com/bong/client/network/FullPowerStateHandler.java:78-79 (readLong calls) with required check at line 80  |  proto/bong/envelope.proto:2960-2965 (message FullPowerExhaustedState { ... uint64 started_tick = 3; uint64 recovery_at_tick = 4; })

### [WARN] `wounds_snapshot` · `wounds[].updated_at_ms` (A_shape)
- **proto 实际输出**:uint64 field on the repeated WoundEntry message → proto3-JSON canonical output is a JSON STRING
- **handler 期望**:readDouble(obj, "updated_at_ms", 0d) requires JsonPrimitive.isNumber()==true; for a quoted string this is false so it always falls back to 0d
- **运行时后果**:every WoundsStore.Wound.updatedAtMs is always 0 instead of the real server timestamp. No current UI consumer of Wound.updatedAtMs() was found in the client codebase (grep returned no hits), so today this is a latent/dormant defect with no observable HUD symptom, but any future feature that sorts wounds by recency or fades in new wounds by timestamp will silently break on day one.
- **证据**:client/src/main/java/com/bong/client/combat/handler/WoundsSnapshotHandler.java:52 (`(long) readDouble(obj, "updated_at_ms", 0d)`), :80-87 (readDouble: `if (!p.isNumber()) return fallback;`)  |  proto/bong/envelope.proto:1431-1438 (`uint64 updated_at_ms = 7;` on WoundEntry)
- **修法**:same int64-as-string tolerant parsing fix as the other uint64 fields; low priority until a real consumer exists

### [WARN] `false_skin_state` · `equipped_at_tick` (A_shape)
- **proto 实际输出**:uint64 → proto3 canonical JSON encodes as JSON STRING
- **handler 期望**:readDouble() helper gates on JsonPrimitive.isNumber() before calling getAsDouble(); string-typed values fail isNumber() and fallback 0d is returned unconditionally, then Math.round()'d into equippedAtTick
- **运行时后果**:FalseSkinHudStateStore.State.equippedAtTick is always 0 on the client; any "time since false-skin equipped" display or expiry-relative-to-equip-tick logic is wrong
- **证据**:client/src/main/java/com/bong/client/combat/handler/FalseSkinStateHandler.java:28 (call site), :90-97 (readDouble def, isNumber() guard at :93)  |  proto/bong/envelope.proto:1822 (equipped_at_tick uint64)
- **修法**:Same fix as vortex_state: accept numeric-looking JSON strings in readDouble() instead of gating on isNumber()

### [INFO] `recipe_unlocked` · `unlocked_at_tick` (A_shape)
- **proto 实际输出**:unlocked_at_tick is `uint64 unlocked_at_tick = 5;` — per proto3-JSON rules this serializes as a JSON string (e.g. "12345"), not a JSON number.
- **handler 期望**:readLong() only extracts a value when `el.getAsJsonPrimitive().isNumber()` is true; a JSON string primitive fails this check and the function silently falls back to its default of 0L.
- **运行时后果**:Distinct from the source.kind defect above (which already drops the whole message before this field matters). If that upstream defect is fixed independently, unlocked_at_tick would still always resolve to 0 instead of the real tick value, since uint64 always arrives as a JSON string here — same string-vs-number mismatch pattern as botany_skill's fields.
- **证据**:client/src/main/java/com/bong/client/network/RecipeUnlockedHandler.java:27 (long unlockedAtTick = readLong(payload, "unlocked_at_tick");) and :68-72 (readLong impl: `if (... || !el.getAsJsonPrimitive().isNumber()) return 0L;`)  |  proto/bong/envelope.proto:1187 (uint64 unlocked_at_tick = 5 in message RecipeUnlocked)
- **修法**:Same fix as botany_skill: make readLong string-tolerant (accept isString() and Long.parseLong) in addition to isNumber().

## RC6 字段名漂移/不存在  · 12 条

### [CRITICAL] `craft_recipe_list` · `recipes[].station` (C_field_name)
- **proto 实际输出**:field does not exist at all in message CraftRecipeEntry (proto/bong/envelope.proto:1111-1121, only id/category/display_name/materials/qi_cost/time_ticks/output/requirements/unlocked, fields 1-9). Confirmed at the source: server/src/schema/proto_convert.rs:2930-2959 `craft_recipe_list_to_proto` builds `bong::CraftRecipeEntry{...}` and never sets a station value (there is no such proto field to set) — even though the Rust source struct `CraftRecipeEntryV1` (server/src/schema/craft.rs:133-149) DOES carry `pub station: Option<String>` with a code comment literally describing this exact regression: '此前漏发本字段 → 制作台配方泄漏到手搓台、点制作报 StationOutOfRange 静默失败' (previously this field was dropped → workbench recipes leak into the handcraft station, clicking craft silently fails with StationOutOfRange).
- **handler 期望**:CraftRecipeListHandler.parseRecipe reads `station = readString(obj, "station")` (client/src/main/java/com/bong/client/network/CraftRecipeListHandler.java:59) and passes it into `new CraftRecipe(...)`. CraftRecipe.isHandcraft() returns `station == null`, isWorkbenchRecipe() returns `"workbench".equals(station)` (client/src/main/java/com/bong/client/craft/CraftRecipe.java:80-86). WorkbenchScreen builds its entire recipe list via `.filter(CraftRecipe::isWorkbenchRecipe)` (client/src/main/java/com/bong/client/craft/WorkbenchScreen.java:90,152); CraftScreen (hand-forge UI) filters via `.filter(CraftRecipe::isHandcraft)` (client/src/main/java/com/bong/client/craft/CraftScreen.java:78,174,191,206,210,213).
- **运行时后果**:`station` is always absent from the proto3-JSON payload, so `readString(obj,"station")` always returns null. WorkbenchScreen's recipe filter (`isWorkbenchRecipe`, station=="workbench") never matches anything → the制作台/workbench crafting screen always shows an empty recipe list. Simultaneously CraftScreen's `isHandcraft` filter (station==null) matches every recipe including workbench-exclusive ones, so workbench-only recipes leak into the hand-forge screen and, per the code's own historical comment, clicking craft on them from there fails silently server-side with StationOutOfRange. This is a live regression of a bug the codebase previously fixed and documented — the proto/bridge layer added later for this payload never carried the fix through.
- **证据**:client/src/main/java/com/bong/client/network/CraftRecipeListHandler.java:59-61; client/src/main/java/com/bong/client/craft/CraftRecipe.java:83,86; client/src/main/java/com/bong/client/craft/WorkbenchScreen.java:90,152; client/src/main/java/com/bong/client/craft/CraftScreen.java:78  |  proto/bong/envelope.proto:1111-1121 (message CraftRecipeEntry, no station field); server/src/schema/proto_convert.rs:2930-2959 (struct literal never sets station); server/src/schema/craft.rs:133-149 (Rust source struct has station: Option<String> with a comment documenting the exact prior incarnation of this bug)
- **修法**:Add `optional string station = 10;` to `message CraftRecipeEntry` in proto/bong/envelope.proto, regenerate, then set `station: r.station.clone()` (or map to the optional wrapper) in server/src/schema/proto_convert.rs:2930-2959; client already reads the right field name so no handler change needed once the proto carries it.

### [CRITICAL] `event_alert` · `severity` (C_field_name)
- **proto 实际输出**:EventAlert has exactly 4 fields: event (EventKind), message (string), zone (optional string), duration_ticks (optional uint64). There is no `severity` field.
- **handler 期望**:Severity.fromWireName(readOptionalString(payload, "severity")) drives toast color (INFO_COLOR/WARNING_COLOR/CRITICAL_COLOR), default duration, and default visual-effect intensity.
- **运行时后果**:readOptionalString always returns null for a field that never exists on the wire → Severity.fromWireName(null) always falls through to WARNING (EventAlertHandler.java:241-249, default branch). Every event_alert toast, regardless of actual urgency, is rendered with WARNING_COLOR/duration/intensity — INFO and CRITICAL styling (and the 0.9 default VFX intensity meant for CRITICAL) can never be selected via this payload.
- **证据**:client/src/main/java/com/bong/client/network/EventAlertHandler.java:40 (readOptionalString(payload,"severity")), 224-266 (Severity enum + fromWireName)  |  proto/bong/envelope.proto:2582-2587 (message EventAlert field list)
- **修法**:Either add a `severity` field to the EventAlert proto message and populate it server-side, or derive severity client-side from the EventKind enum instead of reading a nonexistent wire field.

### [CRITICAL] `ui_open` · `template_id` (C_field_name)
- **proto 实际输出**:UiOpen has exactly 2 fields: `optional string ui = 1` and `string xml = 2`. There is no `template_id` (nor `screen_id`, nor `xml_layout`) field, and the server-side struct it mirrors (ServerDataPayloadV1::UiOpen{ui,xml} in server/src/schema/proto_convert.rs:631) has never had a template concept.
- **handler 期望**:String templateId = readOptionalString(payload, "template_id"); this feeds resolveTemplateOpenState(), the entire template-driven UI-open code path (screenId via "ui" or fallback "screen_id").
- **运行时后果**:readOptionalString(payload,"template_id") is always null on real traffic → normalizedTemplateId is always empty → resolveTemplateOpenState always returns Resolution.failure("") → the entire template-mode branch (gated by BongClientFeatures.ENABLE_XML_TEMPLATE_MODE = true, with real UiOpenScreens template registrations) is permanently unreachable via server_data ui_open pushes. Only the raw dynamic-XML path (`ui` + `xml`) can ever actually open a screen from this channel.
- **证据**:client/src/main/java/com/bong/client/network/UiOpenHandler.java:75-91, 120-145 (resolveTemplateOpenState)  |  proto/bong/envelope.proto:2608-2612 (message UiOpen); server/src/schema/proto_convert.rs:631 (ServerDataPayloadV1::UiOpen { ui, xml } — no template_id ever existed server-side)
- **修法**:Either add a `template_id` field to the UiOpen proto (and the corresponding server-side struct/emit path) if template-driven ui_open is meant to be server-pushable, or remove the dead resolveTemplateOpenState code path from the handler since it can never fire.

### [WARN] `player_state` · `zone_label, zone_spirit_qi` (C_field_name)
- **proto 实际输出**:field absent from the message entirely — PlayerState only declares player/realm/spirit_qi/karma/composite_power/zone/local_neg_pressure/breakdown/season_state/social/spirit_qi_max
- **handler 期望**:readOptionalString(payload,"zone_label") and readOptionalDouble(payload,"zone_spirit_qi",Double.NaN) read fields that never appear in the JSON payload under any name
- **运行时后果**:PlayerStateViewModel.zoneLabel() is always empty/normalized-default and zoneSpiritQiNormalized() is always the NaN-derived clamp default; any UI surface that reads these two derived fields (zone label / zone qi display) never reflects live server data. Gracefully degrades (does not trigger the payload-wide noOp) but the feature is permanently dead on the wire.
- **证据**:client/src/main/java/com/bong/client/network/PlayerStateHandler.java:48,67,69  |  proto/bong/envelope.proto:421-433 (full PlayerState field list, no zone_label/zone_spirit_qi); server/src/schema/proto_convert.rs:598-620 (Rust ServerDataPayloadV1::PlayerState destructure has no such fields either, so this predates the proto migration and is not a reshape regression)
- **修法**:Either add zone_label/zone_spirit_qi to the PlayerState proto message and proto_convert.rs (if the feature is still wanted) or remove the dead reads from PlayerStateHandler.

### [WARN] `techniques_snapshot` · `aliases` (C_field_name)
- **proto 实际输出**:field does not exist — proto message TechniqueEntry (proto/bong/envelope.proto:1559-1574) has no `aliases` field at all, so it never appears in the printed JSON
- **handler 期望**:an optional repeated-string array field named `aliases` on each technique entry object
- **运行时后果**:techniques_snapshot is generic-path (no bridge fixup), and since the proto message truly has no aliases field, `TechniquesListPanel.Technique.aliases()` is always an empty list for every technique on every client. `TechniquesListPanel.matchesQuery()` (client/src/main/java/com/bong/client/combat/inspect/TechniquesListPanel.java:215-217) uses aliases as a fallback search match — alias-based search/filter in the techniques list UI silently never matches anything; only id/display_name substring search works. No crash, no noOp, just a permanently dead search path.
- **证据**:client/src/main/java/com/bong/client/network/TechniquesSnapshotHandler.java:72 (`List<String> aliases = parseAliases(obj);`) and :102-114 (`parseAliases` calls `SkillBarConfigHandler.readArray(obj, "aliases")`, returns `List.of()` when arr is null)  |  proto/bong/envelope.proto:1559-1574 (message TechniqueEntry — fields id/display_name/grade/proficiency/proficiency_label/active/description/required_realm/required_meridians/qi_cost/stamina_cost/cast_ticks/cooldown_ticks/range; no `aliases`); confirmed no `alias` string anywhere in server/src/schema/proto_convert.rs or proto/bong/*.proto via grep
- **修法**:either add `repeated string aliases = 15;` to TechniqueEntry in proto/bong/envelope.proto + populate it server-side in proto_convert.rs, or remove the dead aliases plumbing from TechniquesListPanel/TechniquesSnapshotHandler if the alias-search feature was never actually wired server-side

### [WARN] `combat_event` · `school/style/skill_school, tier, attacker_uuid/source_uuid/caster_uuid, target_uuid/defender_uuid/victim_uuid/entity_uuid, local_player_uuid, victim_name/target_name/entity_name, direction_x/dir_x/dx, direction_z/dir_z/dz, rare_drop/is_rare_drop, kill/is_kill, perfect/perfect_parry` (C_field_name)
- **proto 实际输出**:None of these keys exist on the wire at all — CombatEventFloaterEntry only has kind(string)/amount(float)/text(string)/x/y/z(double).
- **handler 期望**:toJuiceEvent()/juiceKind() read these as optional enrichment fields on each event object to build CombatJuiceEvent (school, attacker/target uuid, direction vector, rare-drop/kill/perfect flags).
- **运行时后果**:CombatJuiceSystem.accept() always builds CombatJuiceEvent with school=CombatSchool.UNSPECIFIED-equivalent default, tier falling back to amount/kind-only heuristic, attacker/target uuid/name always empty string, direction always the hardcoded default (0.0,1.0), rare_drop always false, and kill/perfect flags driven only by the `kind` string switch (not by these boolean fields) — richer per-school VFX/SFX differentiation and rare-drop/perfect-parry juice bonuses can never be triggered via generic combat_event pushes, only the base kind-string switch works.
- **证据**:client/src/main/java/com/bong/client/combat/handler/CombatEventHandler.java:138,139,144-150,157,160 (firstString/firstDouble/readBoolean calls on fields absent from proto)  |  proto/bong/envelope.proto:1831-1838 (message CombatEventFloaterEntry { string kind=1; float amount=2; string text=3; double x=4; double y=5; double z=6; }); server/src/schema/server_data.rs:604-611 (CombatEventFloaterV1/CombatEventFloaterEntryV1 struct has the same 6 fields, no extras); server/src/schema/proto_convert.rs:1343-1358 (1:1 field mapping, confirms no extra fields ever populated)

### [WARN] `heart_demon_offer` · `choices[].alignment / choices[].cost_summary / choices[].cost_flavor` (C_field_name)
- **proto 实际输出**:HeartDemonOfferChoice (proto/bong/envelope.proto:2296-2303) defines exactly 6 fields: choice_id, category, title, effect_summary, flavor, style_hint — no alignment/cost_summary/cost_flavor field exists. Confirmed at the data-model root too: internal Rust struct HeartDemonOfferChoiceV1 (server/src/schema/server_data.rs:842-849) and its proto_convert mapping (server/src/schema/proto_convert.rs:1160-1167, 1147-1170) both construct only those same 6 fields — server never populates alignment/cost data for this message type. So generic-path JSON for a choice object is {choice_id, category, title, effect_summary, flavor, style_hint} only.
- **handler 期望**:HeartDemonOfferHandler.readChoices (client/src/main/java/com/bong/client/network/HeartDemonOfferHandler.java:76-86) builds each InsightChoice by reading choice.get("alignment") via InsightAlignment.parse(readString(choice,"alignment")) (line 79), and choice.get("cost_summary")/choice.get("cost_flavor") (lines 82, 84) as if these were real per-choice wire fields.
- **运行时后果**:getAsJsonPrimitive() lookups for alignment/cost_summary/cost_flavor on every heart-demon choice always miss, so every rendered choice silently gets the same hardcoded fallback text ('代价待结算' / '心魔会索取对应代价。' / InsightAlignment NEUTRAL) regardless of which choice/category was actually offered — no real per-choice cost or alignment tint ever reaches the tribulation heart-demon UI, and this is masked because the fallback text still renders plausibly (no crash, no noOp).
- **证据**:client/src/main/java/com/bong/client/network/HeartDemonOfferHandler.java:76-86 (readString(choice,"alignment"), readString(choice,"cost_summary"), readString(choice,"cost_flavor")); proto/bong/envelope.proto:2296-2303 (HeartDemonOfferChoice message, no such fields); server/src/schema/proto_convert.rs:1160-1167 (choice conversion omits them)  |  proto/bong/envelope.proto:2296-2303
- **修法**:Either add alignment/cost_summary/cost_flavor to HeartDemonOfferChoiceV1 + proto + proto_convert.rs if per-choice cost narrative is intended for heart-demon tribulation offers (matching the richer InsightOffer/InsightChoice model), or drop these three reads from HeartDemonOfferHandler and use InsightChoice's 6-arg convenience constructor (which already supplies the same sensible defaults) to avoid dead field reads.

### [WARN] `event_alert` · `effect` (C_field_name)
- **proto 实际输出**:No `effect` field exists on EventAlert (only event/message/zone/duration_ticks).
- **handler 期望**:parseEffectHint(payload.get("effect"), ...) reads either a string primitive or a nested object with type/hint/name/intensity/duration_ms to build a VisualEffectState.
- **运行时后果**:payload.get("effect") is always null (field never present) → VisualEffectState.none() always → server-driven screen-effect hints attached to event_alert can never render, regardless of what the server intends to signal.
- **证据**:client/src/main/java/com/bong/client/network/EventAlertHandler.java:47, 91-127 (parseEffectHint)  |  proto/bong/envelope.proto:2582-2587
- **修法**:Add an `effect` submessage to EventAlert proto if this is meant to be server-driven, or remove the dead client-side parseEffectHint code path.

### [INFO] `zone_info` · `display_name` (C_field_name)
- **proto 实际输出**:field absent from the message entirely — ZoneInfo only declares zone/spirit_qi/danger_level/status/active_events/perception_text
- **handler 期望**:readOptionalString(payload,"display_name") reads a field that never appears in the JSON payload
- **运行时后果**:ZoneState.displayName is always null; cosmetic-only — UI presumably falls back to the raw zone id when rendering, no crash or noOp since this is an optional constructor argument.
- **证据**:client/src/main/java/com/bong/client/network/ZoneInfoHandler.java:43  |  proto/bong/envelope.proto:336-343 (full ZoneInfo field list, no display_name); server/src/schema/proto_convert.rs:572-586 (Rust ServerDataPayloadV1::ZoneInfo destructure has no display_name field either, predates proto migration)
- **修法**:Either add display_name to the ZoneInfo proto message/proto_convert.rs, or remove the dead read from ZoneInfoHandler.

### [INFO] `craft_session_state` · `error` (C_field_name)
- **proto 实际输出**:message CraftSessionState has no `error` field at all — fields are v/player_id/active/recipe_id/elapsed_ticks/total_ticks/completed_count/total_count/ts (proto/bong/envelope.proto:1132-1142). Confirmed server-side too: server/src/schema/proto_convert.rs:2965-2976 `craft_session_state_to_proto` never sets any error-like field.
- **handler 期望**:CraftSessionStateHandler.java:22 reads `readString(payload, "error")` and threads it into `CraftSessionStateView(...)`.
- **运行时后果**:readString always returns null for a missing key, so CraftSessionStateView.error() is always the empty string. Verified no consumer in the client reads `.error()`/`failureReason`-equivalent for craft_session_state anywhere else (`grep -rn "\.error()" client/src/main/java/com/bong/client/craft/` only hits the getter definition itself) — so today this is a dead field with zero observable effect, but it is a genuine proto/handler drift that will silently do nothing if a future feature (e.g. an in-progress-session error banner) is wired to it.
- **证据**:client/src/main/java/com/bong/client/network/CraftSessionStateHandler.java:22-24; client/src/main/java/com/bong/client/craft/CraftSessionStateView.java:22,43,52  |  proto/bong/envelope.proto:1132-1142 (message CraftSessionState); server/src/schema/proto_convert.rs:2965-2976
- **修法**:Either add an `optional string error` (or a CraftFailureReason enum) field to CraftSessionState in the proto + Rust source, or delete the dead read in CraftSessionStateHandler.

### [INFO] `combat_event` · `color` (C_field_name)
- **proto 实际输出**:Field does not exist on CombatEventFloaterEntry at all.
- **handler 期望**:readDouble(obj, "color", defaultColorFor(kindWire)) — attempts to read a per-event override color, falling back to a hardcoded default keyed by kind.
- **运行时后果**:Damage-floater color is always the hardcoded defaultColorFor(kind) value; any server-side attempt to customize floater color per event is impossible on the current wire — low-impact since the fallback is a sane default, but the field is entirely dead code that will silently never fire.
- **证据**:client/src/main/java/com/bong/client/combat/handler/CombatEventHandler.java:61 (and doc-comment at line 25 describing a "color" field that was never added to the proto)  |  proto/bong/envelope.proto:1831-1838 (message CombatEventFloaterEntry, no color field); server/src/schema/server_data.rs:610-611 (CombatEventFloaterEntryV1 struct, no color field)

### [INFO] `event_alert` · `duration_ms` (C_field_name)
- **proto 实际输出**:No `duration_ms` field exists; only `duration_ticks` (optional uint64, tick-based not ms-based).
- **handler 期望**:readOptionalLong(payload, "duration_ms") is used to override the severity-derived toast duration.
- **运行时后果**:Always null → falls back to severity.defaultDurationMillis(); low practical impact since a sane fallback exists, but the field is genuinely unreachable dead code and cannot be used to fine-tune toast lifetime from the server side.
- **证据**:client/src/main/java/com/bong/client/network/EventAlertHandler.java:41  |  proto/bong/envelope.proto:2586 (duration_ticks only)
- **修法**:Drop the dead duration_ms read, or rename to consume duration_ticks with a tick→ms conversion.

## RC4 proto string 内嵌 JSON (双重编码) vs handler 读对象  · 1 条

### [CRITICAL] `loot_container_open` · `source_kind` (A_shape)
- **proto 实际输出**:proto field is a plain `string` carrying a serde-serialized (externally-tagged) LootContainerSourceKindV1 as a JSON-encoded STRING, e.g. `"source_kind": "{\"supply_coffin\":{\"grade\":\"legendary\"}}"` for SupplyCoffin, or `"source_kind": "\"dead_drop\""` for the unit variant DeadDrop. JsonFormat has no way to know the string embeds JSON, so it is always emitted as a JsonPrimitive string, never a JsonObject
- **handler 期望**:parseSourceKind() branches on sourceKindEl.isJsonObject() to extract sk.has("kind")/"grade", or variant-keyed sub-objects sk.has("supply_coffin")/"storage_crate"/"dead_drop" to recover kind+grade/is_herb
- **运行时后果**:Because source_kind is always a JsonPrimitive string (never a JsonObject), parseSourceKind always takes the isJsonPrimitive branch and returns new SourceKindInfo(sourceKindEl.getAsString(), "common") -- kind becomes the literal raw JSON text (e.g. the string `{"supply_coffin":{"grade":"legendary"}}`) instead of "supply_coffin"/"storage_crate"/"dead_drop", and grade is always hardcoded to "common" (losing the real grade for SupplyCoffin and is_herb for StorageCrate). Lines 104-126 (the isJsonObject/variant-key parsing) are dead code that can never execute given this proto shape. Any UI icon/label/grade-color logic keyed on SourceKindInfo.kind()/grade() will never match a known kind and always show grade "common" regardless of actual coffin rarity or crate type.
- **证据**:client/src/main/java/com/bong/client/network/LootContainerHandler.java:97-127 (parseSourceKind: the isJsonPrimitive branch at :101-103 short-circuits before the isJsonObject/variant-key branches at :104-126 can ever run)  |  proto/bong/envelope.proto:3775 (`string source_kind = 2; // JSON-encoded LootContainerSourceKindV1`); server/src/schema/proto_convert.rs:1386 (`source_kind: serde_json::to_string(&o.source_kind).unwrap_or_default()`); server/src/schema/server_data.rs:652-656 (externally-tagged enum SupplyCoffin{grade}/StorageCrate{is_herb}/DeadDrop)
- **修法**:Either have parseSourceKind re-parse the inner string via JsonParser.parseString(sourceKindEl.getAsString()) before applying the object-shape logic, or have the server flatten LootContainerSourceKindV1 into dedicated proto fields (kind/grade/is_herb) instead of double-JSON-encoding it into a string.

## RC5 其他形状不符 (数组/对象/map/元组)  · 1 条

### [CRITICAL] `recipe_unlocked` · `source.kind` (A_shape)
- **proto 实际输出**:field 4 `source` is type UnlockEventSource which is a `oneof source { string scroll_item_template=1; string mentor_npc_archetype=2; InsightTrigger insight_trigger=3; }`. proto3-JSON for a oneof prints the SET member directly by its own field name at the object's top level, e.g. {"scroll_item_template":"..."} — there is no discriminator field named "kind" at all, nor sub-fields named "item_template"/"npc_archetype"/"trigger".
- **handler 期望**:A nested tagged-union object {"kind":"scroll"|"mentor"|"insight", "item_template"|"npc_archetype"|"trigger":...} — reads sourceObj.get("kind") as the discriminator.
- **运行时后果**:typeString="recipe_unlocked" is not in the 14 specialCased bridge fixups, so it goes through the generic JsonFormat path unmodified. sourceObj.get("kind") is always null for every real message (scroll/mentor/insight), so RecipeUnlockedHandler.handle() always returns noOp. The entire recipe_unlocked feature (残卷/师承/顿悟 recipe unlock notifications) is silently and permanently dropped client-side; CraftStore.recordUnlock is never called for any unlock event.
- **证据**:client/src/main/java/com/bong/client/network/RecipeUnlockedHandler.java:33-38 (String kind = readString(sourceObj, "kind"); if (kind == null) return noOp) and lines 39-46 (switch on kind reading item_template/npc_archetype/trigger)  |  proto/bong/envelope.proto:1145-1151 (message UnlockEventSource, oneof source) and proto/bong/envelope.proto:1186 (UnlockEventSource source = 4 in RecipeUnlocked)
- **修法**:Either add a bridgeRecipeUnlocked fixup in ProtoServerDataBridge that reshapes the oneof into the {kind, ...} JSON the handler expects, or rewrite RecipeUnlockedHandler to read whichever of scroll_item_template/mentor_npc_archetype/insight_trigger key is present directly (oneof-native reading), including stripping the InsightTrigger enum prefix for the insight case.


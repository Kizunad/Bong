package com.bong.client.hud;

import net.minecraft.util.Identifier;

import java.util.EnumMap;
import java.util.EnumSet;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.Set;

/**
 * HUD 表现迁移的唯一盘点表。
 *
 * <p>此处登记当前生产 HUD surface 的责任方和表现路径。它不读取 Store，也不参与
 * 布局或绘制；后续迁移只能改变某个 surface 的路径和表现类型，不能绕过登记直接接线。</p>
 */
public final class HudRenderRegistry {
    private static final List<SurfaceDefinition> PRODUCTION_SURFACES = validate(List.of(
        command(HudRenderLayer.BASELINE, "BongHudOrchestrator", "client_connection", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.ZONE, "ZoneHudRenderer+BongHudOrchestrator", "zone_state+negative_pressure", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.COMPASS, "RetiredHud", "retired", "罗盘 HUD 已移除，不再生成命令"),
        command(HudRenderLayer.THREAT_INDICATOR, "ThreatIndicatorHudPlanner", "player_state+perception+tribulation", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.HUD_VARIANT, "HudEnvironmentVariantPlanner", "zone_state+extract_state", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.TARGET_INFO, "TargetInfoHudPlanner", "target_snapshot+clock", "目标名称仍走 Minecraft GUI"),
        command(HudRenderLayer.MINI_BODY, "MiniBodyHudPlanner", "combat_snapshot+inventory+season", "数值文字仍走 Minecraft GUI"),
        command(HudRenderLayer.QUICK_BAR, "QuickBarHudPlanner+WeaponHotbarHudPlanner", "hotbar+skillbar+cast_state", "物品与技能图标、文字仍走 Minecraft GUI"),
        command(HudRenderLayer.CAST_BAR, "QuickBarHudPlanner", "cast_state", "动态文字不适用"),
        command(HudRenderLayer.EVENT_STREAM, "EventStreamHudPlanner+CombatJuiceHudPlanner", "event_stream+combat_clock", "事件文字仍走 Minecraft GUI"),
        svg(HudRenderLayer.JIEMAI_RING, "JiemaiRingHudPlanner", "defense_window+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.SPELL_VOLUME, "SpellVolumeHudPlanner", "spell_volume_state", "数值文字仍走 Minecraft GUI"),
        command(HudRenderLayer.CARRIER, "CarrierHudPlanner+AnqiHudPlanner", "carrier_state+anqi_state", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.YIDAO, "YidaoHudPlanner", "yidao_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.BOTANY, "BotanyHudPlanner", "botany_projection+clock", "动态文字与物品图标仍走 Minecraft GUI"),
        command(HudRenderLayer.GATHERING, "GatheringProgressHud", "gathering_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.DERIVED_ATTR, "StyleBadgeHudPlanner+DerivedAttrIconHudPlanner", "combat_attributes+loadout", "属性文字与图标仍走 Minecraft GUI"),
        command(HudRenderLayer.TOAST, "ToastHudRenderer+ForgeProgressHudPlanner", "toast_queue+forge_outcome", "通知文字仍走 Minecraft GUI"),
        command(HudRenderLayer.VISUAL, "VisualHudRenderer", "visual_effect_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.SPIRITUAL_SENSE, "RetiredHud", "retired", "灵觉 HUD 已移除，不再生成命令"),
        command(HudRenderLayer.EDGE_FEEDBACK, "EdgeFeedbackHudPlanner+TiandaoPresenceHudPlanner", "combat_feedback+tiandao_presence", "动态文字不适用"),
        svg(HudRenderLayer.STATUS_EFFECTS, "StatusEffectHudPlanner", "status_effects", "状态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.PROCESSING_HUD, "ForgeProgressHudPlanner+AlchemyProgressHudPlanner", "forge_session+alchemy_session", "步骤文字仍走 Minecraft GUI"),
        command(HudRenderLayer.LINGTIAN_OVERLAY, "LingtianOverlayHudPlanner", "lingtian_session+season", "动态文字与物品图标仍走 Minecraft GUI"),
        svg(HudRenderLayer.MOVEMENT_HUD, "MovementHudPlanner", "movement_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.SEARCH_PROGRESS, "SearchProgressHudPlanner", "search_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.DAMAGE_FLOATER, "DamageFloaterHudPlanner", "damage_events+clock", "飘字仍走 Minecraft GUI"),
        command(HudRenderLayer.FLIGHT_HUD, "FlightHudPlanner", "flight_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.COFFIN, "CoffinHudPlanner", "coffin_state", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.TRIBULATION, "TribulationBroadcastHudPlanner", "tribulation_state+clock", "广播文字仍走 Minecraft GUI"),
        command(HudRenderLayer.NEAR_DEATH, "NearDeathOverlayPlanner+NearDeathCollapsePlanner", "combat_state+death_state", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.TSY_EXTRACT, "ExtractProgressHudPlanner", "extract_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.HOME_SEQUENCE, "HomeSequence", "home_sequence+inventory+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.REALM_COLLAPSE, "RealmCollapseHudPlanner", "realm_collapse_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.MERIDIAN_OPEN, "MeridianOpenHudPlanner", "meridian_open_state", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.CONNECTION_STATUS, "ConnectionStatusIndicator", "connection_status+clock", "状态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.VORTEX_CHARGE, "WoliuV2HudPlanner+VortexChargeProgressHud", "vortex_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.VORTEX_COOLDOWN, "WoliuV2HudPlanner+VortexCooldownOverlay", "vortex_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.VORTEX_BACKFIRE, "WoliuV2HudPlanner+BackfireWarningHud", "vortex_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.VORTEX_TURBULENCE, "WoliuV2HudPlanner+TurbulenceFieldVisualizeHud", "vortex_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.DUGU_TAINT_WARNING, "DuguV2HudPlanner", "dugu_state+clock", "动态文字不适用"),
        command(HudRenderLayer.DUGU_TAINT_INDICATOR, "DuguV2HudPlanner", "dugu_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.DUGU_REVEAL_RISK, "DuguV2HudPlanner", "dugu_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.DUGU_SELF_CURE_PROGRESS, "DuguV2HudPlanner", "dugu_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.DUGU_SHROUD, "DuguV2HudPlanner", "dugu_state+clock", "动态文字不适用"),
        command(HudRenderLayer.DUGU_QI_DECAY, "DuguV2HudPlanner", "dugu_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.POISON_TRAIT, "PoisonTraitHudPlanner", "poison_trait_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.DANDAO_MUTATION, "MutationHudPlanner", "dandao_mutation_state", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.SWORD_BOND, "SwordPathHudPlanner", "sword_bond_state+clock", "动态文字仍走 Minecraft GUI"),
        direct(HudRenderLayer.HALLUCINATION, "HALLUCINATION_OVERLAY", "HallucinationHudOverlay", "hallucination_layer+clock", "幻觉文字仍走 Minecraft GUI"),
        command(HudRenderLayer.DYING_ELDER, "DyingElderHudPlanner", "dying_elder_state", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.AGENT_UI, "AgentUiVfxPlanner", "agent_ui_state+clock", "动态文字不适用"),
        command(HudRenderLayer.HALFSTEP_RECHALLENGE, "HalfStepRechallengeHudPlanner", "rechallenge_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.ZHENMAI_PARRY, "ZhenmaiHudPlanner", "zhenmai_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.ZHENMAI_NEUTRALIZE, "ZhenmaiHudPlanner", "zhenmai_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.ZHENMAI_MULTIPOINT, "ZhenmaiHudPlanner", "zhenmai_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.ZHENMAI_HARDEN, "ZhenmaiHudPlanner", "zhenmai_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.ZHENMAI_SEVER, "ZhenmaiHudPlanner", "zhenmai_state+clock", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.SHIELD_BLOCK, "ZhenmaiHudPlanner", "zhenmai_state+clock", "动态文字不适用"),
        command(HudRenderLayer.NICHE_GUARDIAN, "NicheGuardianHudPlanner", "niche_guardian_state", "动态文字仍走 Minecraft GUI"),
        command(HudRenderLayer.MORPH, "MorphHudPlanner", "morph_cast_state+clock", "形态图标与文字仍走 Minecraft GUI"),
        direct(null, "BAOMAI_V3_OVERLAY", "BaomaiV3Hud", "baomai_v3_state+clock", "动态文字仍走 Minecraft GUI"),
        direct(null, "CRACK_READING_OVERLAY", "CrackReadingOverlay", "crack_reading_state+clock", "裂读文字仍走 Minecraft GUI"),
        direct(null, "VOID_EROSION_OVERLAY", "VoidErosionHudOverlay", "void_erosion_state", "阶段文字仍走 Minecraft GUI"),
        direct(null, "RESONANCE_LOCK_OVERLAY", "ResonanceLockMeterHud", "resonance_lock_state+clock", "动态文字不适用")
    ));
    private static final Map<HudRenderLayer, SurfaceDefinition> BY_LAYER = indexByLayer(PRODUCTION_SURFACES);

    private HudRenderRegistry() {
    }

    public static List<SurfaceDefinition> productionSurfaces() {
        return PRODUCTION_SURFACES;
    }

    public static SurfaceDefinition require(HudRenderLayer layer) {
        Objects.requireNonNull(layer, "layer");
        SurfaceDefinition surface = BY_LAYER.get(layer);
        if (surface == null) {
            throw new IllegalArgumentException("未登记的 HUD layer: " + layer);
        }
        return surface;
    }

    public static List<SurfaceDefinition> directOverlays() {
        return PRODUCTION_SURFACES.stream()
            .filter(surface -> surface.path() == RenderPath.DIRECT_OVERLAY)
            .toList();
    }

    private static SurfaceDefinition command(
        HudRenderLayer layer,
        String owner,
        String dynamicBinding,
        String guiException
    ) {
        return new SurfaceDefinition(
            layer.name(),
            Optional.of(layer),
            owner,
            RenderPath.COMMAND,
            Presentation.MINECRAFT_GUI,
            List.of(),
            dynamicBinding,
            guiException,
            "HudRenderRegistryTest"
        );
    }

    private static SurfaceDefinition svg(
        HudRenderLayer layer,
        String owner,
        String dynamicBinding,
        String guiException
    ) {
        return new SurfaceDefinition(
            layer.name(),
            Optional.of(layer),
            owner,
            RenderPath.SVG_FRAME,
            Presentation.SVG_MESH,
            List.of(new SvgAsset("rect", Identifier.of("bong-client", "svg/hud/primitive-rect.svg"))),
            dynamicBinding,
            guiException,
            "HudRenderRegistryTest"
        );
    }

    private static SurfaceDefinition direct(
        HudRenderLayer layer,
        String surfaceId,
        String owner,
        String dynamicBinding,
        String guiException
    ) {
        return new SurfaceDefinition(
            surfaceId,
            Optional.ofNullable(layer),
            owner,
            RenderPath.DIRECT_OVERLAY,
            Presentation.MINECRAFT_GUI,
            List.of(),
            dynamicBinding,
            guiException,
            "HudRenderRegistryTest"
        );
    }

    private static List<SurfaceDefinition> validate(List<SurfaceDefinition> surfaces) {
        Set<String> ids = new HashSet<>();
        EnumSet<HudRenderLayer> layers = EnumSet.noneOf(HudRenderLayer.class);
        int lastLayerOrdinal = -1;
        for (SurfaceDefinition surface : surfaces) {
            if (!ids.add(surface.surfaceId())) {
                throw new IllegalStateException("HUD surface id 重复: " + surface.surfaceId());
            }
            if (surface.layer().isPresent()) {
                HudRenderLayer layer = surface.layer().get();
                if (!layers.add(layer)) {
                    throw new IllegalStateException("HUD layer 重复登记: " + layer);
                }
                if (layer.ordinal() <= lastLayerOrdinal) {
                    throw new IllegalStateException("HUD layer 注册顺序必须与枚举顺序一致: " + layer);
                }
                lastLayerOrdinal = layer.ordinal();
            }
            boolean hasAssets = !surface.svgAssets().isEmpty();
            if ((surface.path() == RenderPath.SVG_FRAME) != hasAssets) {
                throw new IllegalStateException("SVG surface 必须且只能登记 SVG 资源: " + surface.surfaceId());
            }
            Set<String> assetKeys = new HashSet<>();
            for (SvgAsset asset : surface.svgAssets()) {
                if (!assetKeys.add(asset.key())) {
                    throw new IllegalStateException("HUD SVG 资产键重复: " + surface.surfaceId() + "/" + asset.key());
                }
            }
            if (surface.path() == RenderPath.SVG_FRAME && surface.presentation() != Presentation.SVG_MESH) {
                throw new IllegalStateException("SVG frame 必须提交 SVG mesh: " + surface.surfaceId());
            }
            if (surface.path() != RenderPath.SVG_FRAME && surface.presentation() != Presentation.MINECRAFT_GUI) {
                throw new IllegalStateException("非 SVG surface 必须保持 Minecraft GUI 提交: " + surface.surfaceId());
            }
        }
        if (!layers.equals(EnumSet.allOf(HudRenderLayer.class))) {
            EnumSet<HudRenderLayer> missing = EnumSet.allOf(HudRenderLayer.class);
            missing.removeAll(layers);
            throw new IllegalStateException("HUD layer 登记不完整: " + missing);
        }
        return List.copyOf(surfaces);
    }

    private static Map<HudRenderLayer, SurfaceDefinition> indexByLayer(List<SurfaceDefinition> surfaces) {
        Map<HudRenderLayer, SurfaceDefinition> indexed = new EnumMap<>(HudRenderLayer.class);
        for (SurfaceDefinition surface : surfaces) {
            surface.layer().ifPresent(layer -> indexed.put(layer, surface));
        }
        return Map.copyOf(indexed);
    }

    public enum RenderPath {
        COMMAND,
        DIRECT_OVERLAY,
        SVG_FRAME
    }

    public enum Presentation {
        MINECRAFT_GUI,
        SVG_MESH
    }

    /** 一个 surface 可以登记多个 SVG 视觉面，但资源 ID 始终集中于此。 */
    public record SvgAsset(String key, Identifier resource) {
        public SvgAsset {
            if (key == null || !key.matches("[a-z][a-z0-9-]*")) {
                throw new IllegalArgumentException("SVG asset key 必须是小写连字符标识: " + key);
            }
            resource = Objects.requireNonNull(resource, "resource");
        }

        public String fixtureValue() {
            return key + "=" + resource;
        }
    }

    public record SurfaceDefinition(
        String surfaceId,
        Optional<HudRenderLayer> layer,
        String owner,
        RenderPath path,
        Presentation presentation,
        List<SvgAsset> svgAssets,
        String dynamicBinding,
        String guiException,
        String testOwner
    ) {
        public SurfaceDefinition {
            surfaceId = requireText(surfaceId, "surfaceId");
            layer = Objects.requireNonNull(layer, "layer");
            owner = requireText(owner, "owner");
            path = Objects.requireNonNull(path, "path");
            presentation = Objects.requireNonNull(presentation, "presentation");
            svgAssets = List.copyOf(Objects.requireNonNull(svgAssets, "svgAssets"));
            dynamicBinding = requireText(dynamicBinding, "dynamicBinding");
            guiException = requireText(guiException, "guiException");
            testOwner = requireText(testOwner, "testOwner");
        }

        private static String requireText(String value, String field) {
            if (value == null || value.isBlank()) {
                throw new IllegalArgumentException(field + " 不能为空");
            }
            return value;
        }
    }
}

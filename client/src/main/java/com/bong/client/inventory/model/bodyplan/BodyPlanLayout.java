package com.bong.client.inventory.model.bodyplan;

import java.util.List;
import java.util.Locale;

/**
 * plan-race-system-v1 P2b — client 侧 {@code BodyPlanLayoutV1} 镜像。以
 * {@code body_plan_id} 为主键，随 {@code cultivation_detail} 首帧下发的动态部位 /
 * 经脉面板布局元数据（替代 {@code MiniBodyHudPlanner} / {@code BodyInspectComponent}
 * 原先散落的硬编码坐标表）。
 *
 * <p>坐标均归一化到 {@code [0,1]}（原点 = 布局画布左上角），由消费方按自身画布尺寸
 * 缩放推导实际渲染坐标。查找方法均为 O(n) 线性扫描——单个 layout 通常 ≤20
 * 条目，HUD 帧率下可忽略，换取记录类型不可变简单性（record 不支持额外可变缓存字段）。
 *
 * <p>plan-race-system-v1 P2 major 修复 —— {@code hudAnchors} 是可选的第二套锚点组，
 * 专供 {@link com.bong.client.hud.MiniBodyHudPlanner}（30×75 粗网格，宽高比 0.40）
 * 使用，与 {@code anchors}（{@code BodyInspectComponent}，168×236 精细画布，宽高比
 * 0.71）分离——两个消费者画布比例不同，均匀缩放同一套 {@code anchors} 会在 mini HUD
 * 上产生像素漂移。未配置时（非人 plan 常态）为空列表，消费方回退到 {@code anchors}
 * 缩放推导。
 */
public record BodyPlanLayout(
    String bodyPlanId,
    List<SilhouettePart> silhouette,
    List<PartAnchor> anchors,
    List<MeridianPath> meridianPaths,
    List<PartDisplayMapping> partDisplayMap,
    List<PartAnchor> hudAnchors
) {
    public BodyPlanLayout {
        silhouette = List.copyOf(silhouette);
        anchors = List.copyOf(anchors);
        meridianPaths = List.copyOf(meridianPaths);
        partDisplayMap = List.copyOf(partDisplayMap);
        hudAnchors = List.copyOf(hudAnchors);
    }

    /** 四参数构造器兼容旧调用点（未配置 hudAnchors 时留空——非人 plan 的常态）。 */
    public BodyPlanLayout(
        String bodyPlanId,
        List<SilhouettePart> silhouette,
        List<PartAnchor> anchors,
        List<MeridianPath> meridianPaths,
        List<PartDisplayMapping> partDisplayMap
    ) {
        this(bodyPlanId, silhouette, anchors, meridianPaths, partDisplayMap, List.of());
    }

    /** {@code part_id} → {@link PartAnchor}（唯一，layout 校验已在 server 侧保证不重复）。 */
    public PartAnchor anchorFor(String partId) {
        if (partId == null) return null;
        String id = partId.toLowerCase(Locale.ROOT);
        for (PartAnchor anchor : anchors) {
            if (anchor.partId().equalsIgnoreCase(id)) return anchor;
        }
        return null;
    }

    /**
     * {@code part_id} → mini HUD 专用 {@link PartAnchor}（可能为 {@code null}——未配置
     * 该部位，甚至整个 layout 都没有 hud_anchors 段时恒 {@code null}）。
     */
    public PartAnchor hudAnchorFor(String partId) {
        if (partId == null) return null;
        String id = partId.toLowerCase(Locale.ROOT);
        for (PartAnchor anchor : hudAnchors) {
            if (anchor.partId().equalsIgnoreCase(id)) return anchor;
        }
        return null;
    }

    public SilhouettePart silhouetteFor(String partId) {
        if (partId == null) return null;
        String id = partId.toLowerCase(Locale.ROOT);
        for (SilhouettePart part : silhouette) {
            if (part.partId().equalsIgnoreCase(id)) return part;
        }
        return null;
    }

    public MeridianPath meridianPathFor(String channelId) {
        if (channelId == null) return null;
        String id = channelId.toLowerCase(Locale.ROOT);
        for (MeridianPath path : meridianPaths) {
            if (path.channelId().equalsIgnoreCase(id)) return path;
        }
        return null;
    }

    /**
     * server 部位 id → 展示段 id（{@code wounds_snapshot} 的 {@code body_part_wire}
     * 语义镜像）。查不到时原样透传 {@code serverPartId}（非人形 / 未覆盖部位）。
     */
    public String displaySegmentFor(String serverPartId) {
        if (serverPartId == null) return null;
        for (PartDisplayMapping mapping : partDisplayMap) {
            if (mapping.serverPartId().equalsIgnoreCase(serverPartId)) {
                return mapping.displaySegmentId();
            }
        }
        return serverPartId;
    }

    /**
     * 该 layout 是否声明了这个（规范化，通常为 client 16 段展示 id 之一）部位 id——
     * 即出现在 anchors 或 part_display_map 的 display_segment_id 集合中。
     */
    public boolean declaresPart(String canonicalPartId) {
        if (canonicalPartId == null) return false;
        String id = canonicalPartId.toLowerCase(Locale.ROOT);
        if (anchorFor(id) != null) return true;
        for (PartDisplayMapping mapping : partDisplayMap) {
            if (mapping.displaySegmentId().equalsIgnoreCase(id)) return true;
        }
        return false;
    }
}

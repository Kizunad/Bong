package com.bong.client.npc;

/**
 * 远视野低 LOD NPC 快照（server→client 只读渲染数据）。
 *
 * <p>对应 server {@code bong:npc_lod} payload（plan-offscreen-war-v1 §P7 PR-11）。
 *
 * <p><b>守恒红线</b>：此 record 不含任何 qi / ledger 字段，是纯渲染数据，
 * 零真元流动，不触碰 NpcDormantStore / qi_physics。
 *
 * <p>字段对齐 {@code NpcLodS2c}（server Rust struct）：
 * <ul>
 *   <li>{@code entityId} — Valence entity id，与 MC 原生 entity 对齐
 *   <li>{@code archetype} — snake_case（"rogue"/"zombie"/"commoner" 等）
 *   <li>{@code realm} — 中文境界标签（"引气"/"凝脉" 等）
 *   <li>{@code hpRatio} — 血量比率 [0.0, 1.0]，供血量条 overlay
 *   <li>{@code x}/{@code y}/{@code z} — NPC 当前坐标（方便客户端本地距离判断）
 * </ul>
 */
public record NpcLodSnapshot(
    int entityId,
    String archetype,
    String realm,
    float hpRatio,
    double x,
    double y,
    double z
) {
    /** 规范化构造：archetype/realm 不得为 null，hpRatio clamp 到 [0,1]。 */
    public NpcLodSnapshot {
        archetype = archetype == null || archetype.isBlank() ? "unknown" : archetype.trim();
        realm = realm == null || realm.isBlank() ? "醒灵" : realm.trim();
        hpRatio = (float) Math.max(0.0, Math.min(1.0, hpRatio));
    }

    /**
     * 距离平方（XZ 平面，与 server NPC_LOD_SEND_RADIUS 对齐的判断逻辑）。
     *
     * <p>客户端用此值在 {@code NpcLodWorldRenderer} 中做 LOD gate：
     * 只渲染距离玩家 80–256 格的 snapshot。
     */
    public double distanceSquaredXzTo(double px, double pz) {
        double dx = x - px;
        double dz = z - pz;
        return dx * dx + dz * dz;
    }
}

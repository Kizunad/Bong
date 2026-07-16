package com.bong.client.inventory.model.bodyplan;

/**
 * server 部位 id → client 展示段 id 映射（替代
 * {@code network::wounds_snapshot_emit::body_part_wire} 的硬编码 match）。镜像 server
 * {@code BodyPlanPartDisplayMappingV1}。
 */
public record PartDisplayMapping(String serverPartId, String displaySegmentId) {
}

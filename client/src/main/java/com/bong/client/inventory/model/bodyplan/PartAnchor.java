package com.bong.client.inventory.model.bodyplan;

/** 部位锚点（伤口红点位 / 状态图标定位点）。镜像 server {@code BodyPlanPartAnchorV1}。 */
public record PartAnchor(String partId, Point2 point) {
}

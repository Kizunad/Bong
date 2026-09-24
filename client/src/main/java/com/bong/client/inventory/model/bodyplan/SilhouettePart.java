package com.bong.client.inventory.model.bodyplan;

import java.util.List;

/** 单个部位的剪影多边形（顶点归一化坐标，按声明顺序首尾相连）。镜像 server {@code BodyPlanSilhouettePartV1}。 */
public record SilhouettePart(String partId, List<Point2> polygon) {
    public SilhouettePart {
        polygon = List.copyOf(polygon);
    }
}

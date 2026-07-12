package com.bong.client.inventory.model.bodyplan;

import java.util.List;

/** 单条经脉的多段折线路径（替代 client {@code MERIDIAN_PATHS} 硬编码）。镜像 server {@code BodyPlanMeridianPathV1}。 */
public record MeridianPath(String channelId, List<Point2> points) {
    public MeridianPath {
        points = List.copyOf(points);
    }
}

package com.bong.client.inventory.model.bodyplan;

/** 归一化坐标点 {@code [0,1]}（原点 = 布局画布左上角）。镜像 server {@code BodyPlanPoint2V1}。 */
public record Point2(double x, double y) {
}

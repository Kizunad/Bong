package com.bong.client.daozhan;

/**
 * plan-daozhan-v1 P1 — 道伥伪装渲染 Mixin（P3 扩展点，P1 为占位）。
 *
 * <p>P1 实现已通过 {@link com.bong.client.fauna.FaunaModel#getTextureResource} 完成：
 * 道伥处于 Mimicry 态时，GeckoLib FaunaModel 返回 Steve 皮肤纹理替换正常道伥贴图，
 * 实现无名玩家外形伪装。
 *
 * <p>本文件为 P3 完整玩家模型替换预留的扩展点：
 * <ul>
 *   <li>P3 目标：在 {@link net.minecraft.client.render.entity.EntityRenderDispatcher#render}
 *       中拦截 Mimicry 态道伥，替换为真实玩家比例的 {@link net.minecraft.client.render.entity.PlayerEntityRenderer}。
 *   <li>实现路径：注入 {@code @Mixin(EntityRenderDispatcher.class)}，检查 entity 为 FaunaEntity
 *       + DAOXIANG 种类 + {@link DaoZhanDisguiseHandler#isDisguised} 为 true 时取消原始渲染，
 *       改为 {@link FakePlayerEntityRenderer#renderDisguisedDaoZhan}。
 *   <li>激活条件：实现完成后在 {@code bong-client.mixins.json} 的 {@code client} 列表中
 *       追加 {@code "com.bong.client.daozhan.FakePlayerRendererMixin"}（需改 package 或使用
 *       完整路径）。
 * </ul>
 *
 * <p>P1 WSLg 验收标注：道伥 Mimicry 态显示为 Daoxiang 体型 + Steve 皮肤（非标准玩家比例）。
 * 完整玩家比例替换在 P3 实装。
 */
public final class FakePlayerRendererMixin {
    // P1 占位：实际伪装贴图逻辑在 FaunaModel.getTextureResource。
    // P3 激活时此类将改为真正的 @Mixin 并注册到 bong-client.mixins.json。
    private FakePlayerRendererMixin() {
    }
}

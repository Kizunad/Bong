package com.bong.client.scroll;

import java.util.List;
import java.util.Objects;

/**
 * 一次 {@code ScrollOpen} S2C payload 的快照（plan-scroll-reading-v1 P1）。
 *
 * <p>可复用壳的唯一输入：{@link com.bong.client.scroll.ScrollReadScreen} 只依赖此记录的三个
 * 字段渲染，不 hardcode 任何具体残卷内容——下一卷任意 {@code readable_scroll_spec} 挂的物品，
 * server 只需 emit 同一 {@code ScrollOpen} payload，client 零改动即可读。
 */
public record ScrollOpenViewModel(
    String scrollId,      // 模板 id，如 scroll_meridian_primer
    String title,         // 卷名，如《经脉浅述·残卷》
    List<String> bodyPages // 每元素一页，≥1（proto §9 契约）
) {
    public ScrollOpenViewModel {
        Objects.requireNonNull(scrollId, "scrollId");
        Objects.requireNonNull(title, "title");
        Objects.requireNonNull(bodyPages, "bodyPages");
        if (bodyPages.isEmpty()) {
            throw new IllegalArgumentException(
                "bodyPages 不能为空——ScrollOpen 契约(proto §9)要求 body_pages >= 1"
            );
        }
        bodyPages = List.copyOf(bodyPages);
    }

    /** 总页数（>=1，构造时已校验）。 */
    public int pageCount() {
        return bodyPages.size();
    }

    /** 把任意 index 钳到 [0, pageCount()-1] 合法范围内（负数/越界均钳到最近边界）。 */
    public int clampPageIndex(int index) {
        return Math.max(0, Math.min(index, pageCount() - 1));
    }

    /** 取第 index 页正文（index 会先被 {@link #clampPageIndex(int)} 钳边界，永不抛异常）。 */
    public String page(int index) {
        return bodyPages.get(clampPageIndex(index));
    }

    /** 是否有下一页（index 是当前页，从 0 起）。 */
    public boolean hasNextPage(int index) {
        return index < pageCount() - 1;
    }

    /** 是否有上一页（index 是当前页，从 0 起）。 */
    public boolean hasPrevPage(int index) {
        return index > 0;
    }
}

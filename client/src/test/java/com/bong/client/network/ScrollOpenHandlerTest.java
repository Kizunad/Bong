package com.bong.client.network;

import com.bong.client.scroll.ScrollOpenViewModel;
import com.bong.client.scroll.ScrollReadStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.*;

/**
 * ScrollOpenHandler 单测（plan-scroll-reading-v1 P1）。
 *
 * <p>测试策略：
 * <ul>
 *   <li>正常 payload → ScrollReadStore 快照字段断言（scrollId/title/bodyPages）。</li>
 *   <li>多页 payload 正确 round-trip。</li>
 *   <li>畸形 payload 守卫：缺 scroll_id / 缺 title / 缺 body_pages / body_pages 为空数组 /
 *       body_pages 非数组类型 → 均 noOp 且 store 不写入（"空页拒绝"）。</li>
 *   <li>ServerDataRouter.createDefault() 注册了 "scroll_open"。</li>
 * </ul>
 */
public class ScrollOpenHandlerTest {

    @AfterEach
    void resetStore() {
        ScrollReadStore.resetForTests();
    }

    // ─── 正常 payload：store 被填 ───────────────────────────────────────────

    @Test
    void normalPayload_populatesScrollReadStore() {
        String json = """
            {
              "v": 1,
              "type": "scroll_open",
              "scroll_id": "scroll_meridian_primer",
              "title": "经脉浅述·残卷",
              "body_pages": ["第一页正文", "第二页正文", "第三页正文"]
            }
            """;
        ServerDataDispatch dispatch = new ScrollOpenHandler().handle(parse(json));

        assertTrue(dispatch.handled(),
            "正常 payload 应返回 dispatch.handled()==true，实际=" + dispatch.handled());

        ScrollOpenViewModel offer = ScrollReadStore.snapshot();
        assertNotNull(offer, "handler 应将 ScrollOpen 写入 ScrollReadStore，snapshot 不应为 null");
        assertEquals("scroll_meridian_primer", offer.scrollId(),
            "scrollId 应 round-trip，实际=" + offer.scrollId());
        assertEquals("经脉浅述·残卷", offer.title(),
            "title 应 round-trip，实际=" + offer.title());
        assertEquals(3, offer.bodyPages().size(),
            "bodyPages 大小应为 3，实际=" + offer.bodyPages().size());
        assertEquals("第一页正文", offer.bodyPages().get(0));
        assertEquals("第二页正文", offer.bodyPages().get(1));
        assertEquals("第三页正文", offer.bodyPages().get(2));
    }

    @Test
    void singlePagePayload_populatesStoreWithOnePage() {
        String json = """
            {
              "v": 1,
              "type": "scroll_open",
              "scroll_id": "scroll_single",
              "title": "单页残卷",
              "body_pages": ["唯一一页"]
            }
            """;
        new ScrollOpenHandler().handle(parse(json));

        ScrollOpenViewModel offer = ScrollReadStore.snapshot();
        assertNotNull(offer);
        assertEquals(1, offer.bodyPages().size(),
            "单页 payload 应产生 pageCount=1，实际=" + offer.bodyPages().size());
    }

    // ─── 畸形 payload 守卫 ───────────────────────────────────────────────

    @Test
    void missingScrollId_returnsNoOp() {
        String json = """
            {
              "v": 1,
              "type": "scroll_open",
              "title": "无 id 残卷",
              "body_pages": ["正文"]
            }
            """;
        ServerDataDispatch dispatch = new ScrollOpenHandler().handle(parse(json));

        assertFalse(dispatch.handled(),
            "缺 scroll_id 应返回 noOp（handled=false），实际=" + dispatch.handled());
        assertNull(ScrollReadStore.snapshot(),
            "缺 scroll_id 时 ScrollReadStore 不应被写入，snapshot 应为 null");
    }

    @Test
    void blankScrollId_returnsNoOp() {
        String json = """
            {
              "v": 1,
              "type": "scroll_open",
              "scroll_id": "   ",
              "title": "空白 id 残卷",
              "body_pages": ["正文"]
            }
            """;
        ServerDataDispatch dispatch = new ScrollOpenHandler().handle(parse(json));

        assertFalse(dispatch.handled(), "空白 scroll_id 应返回 noOp，实际=" + dispatch.handled());
        assertNull(ScrollReadStore.snapshot(), "空白 scroll_id 时 store 不应被写入");
    }

    @Test
    void missingTitle_returnsNoOp() {
        String json = """
            {
              "v": 1,
              "type": "scroll_open",
              "scroll_id": "scroll_no_title",
              "body_pages": ["正文"]
            }
            """;
        ServerDataDispatch dispatch = new ScrollOpenHandler().handle(parse(json));

        assertFalse(dispatch.handled(), "缺 title 应返回 noOp，实际=" + dispatch.handled());
        assertNull(ScrollReadStore.snapshot(), "缺 title 时 store 不应被写入");
    }

    @Test
    void blankTitle_returnsNoOp() {
        String json = """
            {
              "v": 1,
              "type": "scroll_open",
              "scroll_id": "scroll_blank_title",
              "title": "   ",
              "body_pages": ["正文"]
            }
            """;
        ServerDataDispatch dispatch = new ScrollOpenHandler().handle(parse(json));

        assertFalse(dispatch.handled(), "空白 title 应返回 noOp，实际=" + dispatch.handled());
        assertNull(ScrollReadStore.snapshot(), "空白 title 时 store 不应被写入");
    }

    @Test
    void missingBodyPages_returnsNoOp() {
        String json = """
            {
              "v": 1,
              "type": "scroll_open",
              "scroll_id": "scroll_no_pages",
              "title": "无正文残卷"
            }
            """;
        ServerDataDispatch dispatch = new ScrollOpenHandler().handle(parse(json));

        assertFalse(dispatch.handled(), "缺 body_pages 字段应返回 noOp，实际=" + dispatch.handled());
        assertNull(ScrollReadStore.snapshot(), "缺 body_pages 时 store 不应被写入");
    }

    @Test
    void emptyBodyPagesArray_returnsNoOp() {
        // "空页拒绝" 边界：body_pages 是合法数组类型但长度为 0。
        String json = """
            {
              "v": 1,
              "type": "scroll_open",
              "scroll_id": "scroll_empty_pages",
              "title": "空正文残卷",
              "body_pages": []
            }
            """;
        ServerDataDispatch dispatch = new ScrollOpenHandler().handle(parse(json));

        assertFalse(dispatch.handled(), "body_pages=[] 应返回 noOp，实际=" + dispatch.handled());
        assertNull(ScrollReadStore.snapshot(), "body_pages 为空数组时 store 不应被写入");
    }

    @Test
    void bodyPagesNotAnArray_returnsNoOp() {
        // body_pages 是字符串而非数组——类型错误也应静默拒绝，而不是抛异常。
        String json = """
            {
              "v": 1,
              "type": "scroll_open",
              "scroll_id": "scroll_wrong_type",
              "title": "类型错误残卷",
              "body_pages": "不是数组"
            }
            """;
        ServerDataDispatch dispatch = new ScrollOpenHandler().handle(parse(json));

        assertFalse(dispatch.handled(), "body_pages 非数组类型应返回 noOp，实际=" + dispatch.handled());
        assertNull(ScrollReadStore.snapshot(), "body_pages 类型错误时 store 不应被写入");
    }

    @Test
    void bodyPagesWithNonStringElements_filtersThemOut() {
        // 数组里混了非字符串元素（如数字）：应被过滤掉，只保留合法字符串页。
        String json = """
            {
              "v": 1,
              "type": "scroll_open",
              "scroll_id": "scroll_mixed_pages",
              "title": "混杂类型残卷",
              "body_pages": ["合法页", 123, "另一合法页"]
            }
            """;
        new ScrollOpenHandler().handle(parse(json));

        ScrollOpenViewModel offer = ScrollReadStore.snapshot();
        assertNotNull(offer, "过滤非法元素后仍应剩 2 页合法正文，不应整体拒绝");
        assertEquals(2, offer.bodyPages().size(),
            "非字符串元素应被过滤，剩余合法页数应为 2，实际=" + offer.bodyPages().size());
    }

    @Test
    void defaultRouter_registersScrollOpen() {
        assertTrue(ServerDataRouter.createDefault().registeredTypes().contains("scroll_open"),
            "ServerDataRouter.createDefault() 应注册 'scroll_open' type");
    }

    // ─── helper ──────────────────────────────────────────────────────────

    private static ServerDataEnvelope parse(String json) {
        return ServerDataEnvelope
            .parse(json, json.getBytes(StandardCharsets.UTF_8).length)
            .envelope();
    }
}

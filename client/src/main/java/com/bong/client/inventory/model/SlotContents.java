package com.bong.client.inventory.model;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * plan-layered-equip-v1 P4（决议 #1/#12/#17）：客户端装备槽内容，镜像 server {@code SlotContents{worn:Vec, held:Option}}。
 *
 * <p>语义：
 * <ul>
 *   <li>{@code worn} = 穿戴件**栈**（LIFO，决议 #12）：栈底 {@code worn.get(0)} 在最下，栈顶 {@code worn.last()}
 *       在最上、唯一可拖/可卸；护甲 / 伪皮 / 背包件同槽叠放（决议 #17）。</li>
 *   <li>{@code held} = 持械单件（手槽武器 / 工具 / 盾 / 法宝）；身体槽通常无 held。</li>
 * </ul>
 *
 * <p>不可变值对象：内部 list 复制 + unmodifiable，避免外部篡改栈序。
 */
public final class SlotContents {
    private final List<InventoryItem> worn;
    private final InventoryItem held;

    private static final SlotContents EMPTY = new SlotContents(List.of(), null);

    public SlotContents(List<InventoryItem> worn, InventoryItem held) {
        List<InventoryItem> copy = new ArrayList<>();
        if (worn != null) {
            for (InventoryItem item : worn) {
                if (item != null && !item.isEmpty()) {
                    copy.add(item);
                }
            }
        }
        this.worn = Collections.unmodifiableList(copy);
        this.held = (held != null && !held.isEmpty()) ? held : null;
    }

    /** 空槽（无 worn 无 held）。 */
    public static SlotContents empty() {
        return EMPTY;
    }

    /** 单 worn 件槽（身体槽常见，如单层护甲）。 */
    public static SlotContents ofWorn(InventoryItem item) {
        return new SlotContents(item == null ? List.of() : List.of(item), null);
    }

    /** 单 held 件槽（手槽持械）。 */
    public static SlotContents ofHeld(InventoryItem item) {
        return new SlotContents(List.of(), item);
    }

    /** worn 栈（栈底 → 栈顶）。不可变。 */
    public List<InventoryItem> worn() {
        return worn;
    }

    /** 持械件（可能为 null）。 */
    public InventoryItem held() {
        return held;
    }

    /** 栈顶 worn 件（决议 #12：唯一可拖/可卸的一层），无则 null。 */
    public InventoryItem wornTop() {
        return worn.isEmpty() ? null : worn.get(worn.size() - 1);
    }

    /** worn 层数。 */
    public int wornCount() {
        return worn.size();
    }

    public boolean isEmpty() {
        return worn.isEmpty() && held == null;
    }

    /**
     * 该槽的「代表件」：手槽 = held，身体槽 = worn 栈顶。供旧单件渲染路径（如手持/护甲模型同步）取数。
     */
    public InventoryItem representative() {
        if (held != null) return held;
        return wornTop();
    }
}

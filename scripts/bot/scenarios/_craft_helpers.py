"""通过真实库存实例逐叠放入制作材料，不绕过服务端托管事务。"""

from bot.scenarios._inventory_helpers import (
    find_item,
    latest_inventory_snapshot,
    wait_inventory_revision_after_matching,
)


def stage_material(bot, recipe_id: str, item_id: str, station_pos=None) -> dict:
    snapshot = latest_inventory_snapshot(bot)
    source = find_item(snapshot, item_id)
    assert source is not None, f"背包缺少待放入的材料 {item_id}"
    instance_id = source["item"]["instance_id"]
    bot.intent({
        "type": "material_move",
        "v": 1,
        "recipe_id": recipe_id,
        "instance_id": instance_id,
        "station_pos": list(station_pos) if station_pos is not None else None,
        "returning": False,
        "expected_revision": snapshot["revision"],
    })
    prepared = wait_inventory_revision_after_matching(
        bot,
        snapshot["revision"],
        lambda state: any(
            item["instance_id"] == instance_id
            for item in state.get("material_preparation", {}).get("materials", [])
        ),
        f"实例 {instance_id} 已移入制作区",
    )
    expected_station = list(station_pos) if station_pos is not None else None
    assert prepared["material_preparation"]["station_pos"] == expected_station, "快照必须保留材料所属工位"
    return prepared

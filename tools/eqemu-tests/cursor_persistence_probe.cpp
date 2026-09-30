// SPDX-License-Identifier: GPL-3.0-or-later
// Actual EQEmu save methods linked against a fail-closed, in-memory SQL seam.
#include "common/shareddb.h"
#include "common/inventory_profile.h"
#include "common/eqemu_config.h"
#include <iostream>
#include <algorithm>
#include <vector>
#include <map>
#include <regex>
#include <stdexcept>
#include <tuple>
const EQEmuConfig* Config = nullptr;
struct Row { uint32_t item; uint16_t charges; };
static std::map<int, Row> rows;
static std::vector<std::tuple<int,uint32_t,uint32_t>> overwritten;
static std::map<uint32_t, EQ::ItemData> definitions;
static bool fail_clear = false;
static int fail_write_slot = -1;
static size_t query_count = 0;
static void require(bool ok, const char* message) { if (!ok) throw std::runtime_error(message); }
extern "C" MYSQL* __wrap_mysql_real_connect(MYSQL*, const char*, const char*, const char*, const char*, unsigned int, const char*, unsigned long) {
    throw std::runtime_error("database connection forbidden in fixture");
}
extern "C" MySQLRequestResult __wrap__ZN6DBcore13QueryDatabaseERKNSt7__cxx1112basic_stringIcSt11char_traitsIcESaIcEEEb(DBcore*, const std::string& query, bool) {
    ++query_count;
    static const std::regex clear(R"(DELETE FROM inventory WHERE `character_id` = 424242 AND \(`slot_id` = (\d+) OR `slot_id` BETWEEN (\d+) AND (\d+)\))");
    static const std::regex single(R"(DELETE FROM inventory WHERE `character_id` = 424242 AND `slot_id` = (\d+))");
    static const std::regex range(R"(DELETE FROM inventory WHERE `character_id` = 424242 AND `slot_id` BETWEEN (\d+) AND (\d+))");
    static const std::regex replace(R"(REPLACE INTO inventory \([a-z_, ]+\) +VALUES \(424242,(\d+),(\d+),(\d+),[0-9,']+\))");
    std::smatch match;
    size_t changed = 0;
    if (std::regex_match(query,match,clear)) {
        if (fail_clear) return MySQLRequestResult();
        int head=std::stoi(match[1]), low=std::stoi(match[2]), high=std::stoi(match[3]);
        for (auto it=rows.begin(); it!=rows.end();) {
            if (it->first==head || (it->first>=low && it->first<=high)) { it=rows.erase(it); ++changed; }
            else ++it;
        }
    } else if (std::regex_match(query,match,single)) {
        changed=rows.erase(std::stoi(match[1]));
    } else if (std::regex_match(query,match,range)) {
        int low=std::stoi(match[1]), high=std::stoi(match[2]);
        for (auto it=rows.begin(); it!=rows.end();) {
            if (it->first>=low && it->first<=high) { it=rows.erase(it); ++changed; } else ++it;
        }
    } else if (std::regex_match(query,match,replace)) {
        int slot=std::stoi(match[1]);
        if (slot==fail_write_slot) return MySQLRequestResult();
        changed=rows.contains(slot)?2:1;
        if (rows.contains(slot)) overwritten.emplace_back(slot,rows.at(slot).item,std::stoul(match[2]));
        rows[slot]={static_cast<uint32_t>(std::stoul(match[2])),static_cast<uint16_t>(std::stoul(match[3]))};
    } else {
        throw std::runtime_error("unexpected fixture query: "+query);
    }
    return MySQLRequestResult(nullptr,changed);
}
static EQ::ItemInstance item(uint32_t id, bool bag=false) {
    auto [it,inserted]=definitions.try_emplace(id);
    if (inserted) {
        auto& data=it->second;
        data.ID=id; data.ItemClass=bag?EQ::item::ItemClassBag:EQ::item::ItemClassCommon;
        data.StackSize=1; data.BagSlots=bag?2:0; data.BagSize=10;
    }
    return EQ::ItemInstance(&it->second,1);
}
static void reset() { rows.clear(); overwritten.clear(); fail_clear=false; fail_write_slot=-1; }
static void require_children(const EQ::ItemInstance* bag, uint32_t a, uint32_t b) {
    require(bag && bag->IsClassBag() && bag->GetItem(0) && bag->GetItem(1)
        && bag->GetItem(0)->GetID()==a && bag->GetItem(1)->GetID()==b,
        "fixture bag children missing before persistence");
}
static bool save(SharedDatabase& db, EQ::InventoryProfile& inventory, size_t* remaining=nullptr) {
    auto begin=inventory.cursor_cbegin(), end=inventory.cursor_cend();
    bool ok=db.SaveCursor(424242,begin,end);
    if (remaining) *remaining=std::distance(begin,end);
    return ok;
}
static size_t roots(EQ::InventoryProfile& inventory) {
    return std::distance(inventory.cursor_cbegin(),inventory.cursor_cend());
}
// Only the exact source slot-dispatch boundary of GetInventory is replayed.
// This does not invoke its full SQL/item-definition/login pipeline.
static void reload_dispatch(EQ::InventoryProfile& inventory) {
    inventory.SetInventoryVersion(EQ::versions::ClientVersion::RoF2);
    for (const auto& [slot,row]:rows) {
        EQ::ItemInstance inst(&definitions.at(row.item),row.charges);
        if (slot>=EQ::invbag::CURSOR_BAG_BEGIN && slot<=EQ::invbag::CURSOR_BAG_END)
            inventory.PushCursor(inst);
        else inventory.PutItem(slot,inst);
    }
}
int main() {
    try {
        SharedDatabase db;
        const int base=EQ::invbag::CURSOR_BAG_BEGIN;
        {
            reset(); EQ::InventoryProfile inv;
            for (uint32_t id=1000; id<1201; ++id) inv.PushCursor(item(id));
            size_t remaining=0;
            require(save(db,inv,&remaining),"overflow did not claim success");
            require(roots(inv)==201 && rows.size()==200 && remaining==1,"unexpected overflow footprint");
            std::cout<<"REPRO overflow: success=true memory_roots=201 saved_roots=200 iterator_remaining=1\n";
        }
        {
            reset(); EQ::InventoryProfile inv;
            auto bag=item(2000,true); bag.PutItem(0,item(2001)); bag.PutItem(1,item(2002));
            inv.PushCursor(item(2003)); inv.PushCursor(bag);
            auto input_tail=inv.cursor_cbegin(); ++input_tail;
            require_children(*input_tail,2001,2002);
            require(save(db,inv),"tail save failed");
            require(rows.size()==2 && rows.at(base+1).item==2000,"unexpected tail footprint");
            EQ::InventoryProfile loaded; reload_dispatch(loaded);
            require(roots(loaded)==2,"unexpected loaded root count");
            auto tail=loaded.cursor_cbegin(); ++tail;
            require(roots(loaded)==2 && (*tail)->IsClassBag() && !(*tail)->GetItem(0) && !(*tail)->GetItem(1),"tail bag unexpectedly retained children");
            std::cout<<"REPRO queued_bag: success=true saved_roots=2 saved_children=0 reloaded_bag_empty=true\n";
        }
        {
            reset(); EQ::InventoryProfile inv;
            auto bag=item(3000,true); bag.PutItem(0,item(3001)); bag.PutItem(1,item(3002));
            inv.PushCursor(bag); inv.PushCursor(item(3003));
            require_children(inv.GetItem(EQ::invslot::slotCursor),3001,3002);
            require(save(db,inv),"headbag save failed");
            require(rows.size()==3 && rows.at(base).item==3001 && rows.at(base+1).item==3003,"head child/tail collision not reproduced");
            require(std::find(overwritten.begin(),overwritten.end(),std::make_tuple(base+1,uint32_t(3002),uint32_t(3003)))!=overwritten.end(),"tail did not overwrite saved child1");
            EQ::InventoryProfile loaded; reload_dispatch(loaded);
            require(roots(loaded)==3 && loaded.GetItem(EQ::invslot::slotCursor)->IsClassBag() && !loaded.GetItem(EQ::invslot::slotCursor)->GetItem(0),"loader routing mismatch");
            std::cout<<"REPRO head_bag_tail_collision: success=true child1_overwritten=true loaded_roots=3 head_bag_empty=true child0_is_loose_root=true\n";
        }
        {
            reset(); EQ::InventoryProfile inv;
            inv.PushCursor(item(4000)); rows[base+100]={4001,1}; fail_clear=true;
            require(save(db,inv) && rows.contains(base+100),"clear failure was propagated");
            std::cout<<"REPRO clear_failure: success=true stale_tail_preserved=true\n";
        }
        {
            reset(); EQ::InventoryProfile inv;
            auto bag=item(5000,true); bag.PutItem(0,item(5001)); inv.PushCursor(bag);
            fail_write_slot=base;
            require(save(db,inv) && rows.size()==1 && rows.at(EQ::invslot::slotCursor).item==5000,"child failure was propagated");
            std::cout<<"REPRO child_write_failure: success=true child_missing=true\n";
        }
        EQ::InventoryProfile::CleanDirty();
        std::cout<<"PASS5 actual_source_methods=true intercepted_queries="<<query_count<<" database_connections=0 live_sql=0\n";
        return 0;
    } catch (const std::exception& e) { std::cerr<<"FAIL "<<e.what()<<'\n'; return 1; }
}

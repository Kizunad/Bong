use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::social::components::IntrusionRecord;
use crate::social::events::NicheIntrusionEvent;
use valence::prelude::{App, EventReader, Query, Update};

pub fn register(app: &mut App) {
    app.add_systems(Update, write_niche_intrusion_events_to_life_records);
}

pub fn write_intrusion_record(
    owner_life: &mut LifeRecord,
    intruder_life: &mut LifeRecord,
    record: &IntrusionRecord,
) {
    let (owner_entry, intruder_entry) = intrusion_entries(
        record.owner.clone(),
        record.intruder_char_id.clone(),
        record.niche_pos,
        record.items_taken.len() as u32,
        record.time,
    );
    owner_life.push(owner_entry);
    intruder_life.push(intruder_entry);
}

fn write_niche_intrusion_events_to_life_records(
    mut events: EventReader<NicheIntrusionEvent>,
    mut life_records: Query<&mut LifeRecord>,
) {
    for event in events.read() {
        let (owner_entry, intruder_entry) = intrusion_entries(
            event.niche_owner.clone(),
            event.intruder_char_id.clone(),
            event.niche_pos,
            event.items_taken.len() as u32,
            event.tick,
        );
        for mut life_record in life_records.iter_mut() {
            if life_record.character_id == event.niche_owner {
                life_record.push(owner_entry.clone());
            } else if life_record.character_id == event.intruder_char_id {
                life_record.push(intruder_entry.clone());
            }
        }
    }
}

fn intrusion_entries(
    owner_id: String,
    intruder_id: String,
    niche_pos: [i32; 3],
    items_taken_count: u32,
    tick: u64,
) -> (BiographyEntry, BiographyEntry) {
    let owner_entry = BiographyEntry::NicheIntrusion {
        owner_id: owner_id.clone(),
        intruder_id: intruder_id.clone(),
        niche_pos,
        items_taken_count,
        owner_perspective: true,
        tick,
    };
    let intruder_entry = BiographyEntry::NicheIntrusion {
        owner_id,
        intruder_id,
        niche_pos,
        items_taken_count,
        owner_perspective: false,
        tick,
    };
    (owner_entry, intruder_entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::Entity;

    #[test]
    fn write_intrusion_record_adds_owner_and_intruder_biography_entries() {
        let mut owner = LifeRecord::new("char:owner".to_string());
        let mut intruder = LifeRecord::new("char:intruder".to_string());
        let record = IntrusionRecord {
            intruder: Entity::from_raw(7),
            intruder_char_id: "char:intruder".to_string(),
            owner: "char:owner".to_string(),
            time: 42,
            niche_pos: [1, 64, 2],
            items_taken: vec![10, 11],
            guardian_kinds_triggered: Vec::new(),
        };

        write_intrusion_record(&mut owner, &mut intruder, &record);

        assert_eq!(owner.biography.len(), 1);
        assert_eq!(intruder.biography.len(), 1);
        assert!(matches!(
            owner.biography[0],
            BiographyEntry::NicheIntrusion {
                owner_perspective: true,
                items_taken_count: 2,
                ..
            }
        ));
        assert!(matches!(
            intruder.biography[0],
            BiographyEntry::NicheIntrusion {
                owner_perspective: false,
                items_taken_count: 2,
                ..
            }
        ));
    }
}

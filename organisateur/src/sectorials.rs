use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Sectorial {
    pub id: u32,
    pub parent_id: u32,
    pub name: String,
    pub slug: String,
    pub short_name: String,
    pub discontinued: bool,
    pub faction: String,
}

pub fn all() -> &'static [Sectorial] {
    use std::sync::OnceLock;
    static CACHE: OnceLock<Vec<Sectorial>> = OnceLock::new();
    CACHE.get_or_init(|| {
        serde_json::from_str(include_str!("../data/sectorials.json"))
            .expect("liste des sectorielles illisible")
    })
}

pub fn by_id(id: u32) -> Option<&'static Sectorial> {
    all().iter().find(|sectorial| sectorial.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_matches_the_playable_list() {
        let list = all();
        assert!(list.len() > 30);
        assert!(list.iter().any(|sectorial| sectorial.slug == "panoceania"));
        assert!(list.iter().all(|sectorial| sectorial.id % 100 != 99));
        assert!(list
            .iter()
            .all(|sectorial| sectorial.slug != "non-aligned-armies"));
        assert!(list
            .iter()
            .all(|sectorial| sectorial.slug != "contracted-back-up"));
        assert!(by_id(101).is_some());
        assert!(by_id(199).is_none());
    }
}

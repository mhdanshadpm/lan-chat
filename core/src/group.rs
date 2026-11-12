//! Pure group helpers: membership, naming, mention parsing. Keeping these pure
//! makes group semantics testable without sockets or a database.

use uuid::Uuid;

use crate::protocol::EVERYONE_GROUP_ID;
use crate::store::{GroupRecord, PeerSnapshot};

pub const MAX_GROUP_NAME_CHARS: usize = 64;
pub const MAX_GROUP_MEMBERS: usize = 256;

/// The implicit "Everyone" group always contains the local peer plus every
/// known peer. It is never sent over the wire; every instance derives it.
pub fn everyone_group(me: Uuid, peers: &[PeerSnapshot]) -> GroupRecord {
    let mut members = vec![me];
    for peer in peers {
        if peer.id != me && !members.contains(&peer.id) {
            members.push(peer.id);
        }
    }
    GroupRecord {
        id: EVERYONE_GROUP_ID,
        name: "Everyone".into(),
        avatar_color: 0x74AA_F7,
        members,
        created_by: me,
        created_at: 0,
        is_default: true,
        version: 0,
    }
}

pub fn sanitize_group_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(MAX_GROUP_NAME_CHARS).collect())
}

/// Extracts peer mentions from text. Supports `@Display Name` (longest match
/// wins, case-insensitive) and `@all` / `@everyone` for the whole group.
pub fn extract_mentions(text: &str, candidates: &[(Uuid, String)]) -> Vec<Uuid> {
    let lower = text.to_lowercase();
    let mut found: Vec<Uuid> = Vec::new();
    let mut sorted: Vec<&(Uuid, String)> = candidates.iter().collect();
    sorted.sort_by_key(|(_, name)| std::cmp::Reverse(name.chars().count()));

    for (id, name) in sorted {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let needle = format!("@{}", name.to_lowercase());
        let mut from = 0;
        while let Some(pos) = lower[from..].find(&needle) {
            let start = from + pos;
            let end = start + needle.len();
            let boundary = lower[end..]
                .chars()
                .next()
                .map(|c| !c.is_alphanumeric() && c != '_' && c != '-')
                .unwrap_or(true);
            if boundary && !found.contains(id) {
                found.push(*id);
            }
            from = end;
            if from >= lower.len() {
                break;
            }
        }
    }

    if lower.contains("@all") || lower.contains("@everyone") {
        for (id, _) in candidates {
            if !found.contains(id) {
                found.push(*id);
            }
        }
    }
    found
}

/// Deterministic ordering used by the UI member list.
pub fn sort_members(members: &mut [Uuid]) {
    members.sort();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everyone_includes_self_and_peers_once() {
        let me = Uuid::new_v4();
        let a = PeerSnapshot::placeholder(Uuid::new_v4());
        let group = everyone_group(me, std::slice::from_ref(&a));
        assert_eq!(group.id, EVERYONE_GROUP_ID);
        assert!(group.is_default);
        assert_eq!(group.members, vec![me, a.id]);
    }

    #[test]
    fn mentions_match_names_with_spaces_and_boundaries() {
        let alice = Uuid::new_v4();
        let bob = Uuid::new_v4();
        let candidates = vec![
            (alice, "Alice Smith".to_string()),
            (bob, "Bob".to_string()),
        ];
        assert_eq!(
            extract_mentions("hey @Alice Smith can you look?", &candidates),
            vec![alice]
        );
        assert_eq!(extract_mentions("@bob@bob", &candidates), vec![bob]);
        assert!(extract_mentions("@Bobby", &candidates).is_empty());
        assert_eq!(extract_mentions("@all standup", &candidates).len(), 2);
    }

    #[test]
    fn group_names_are_trimmed_and_rejected_when_empty() {
        assert_eq!(sanitize_group_name("  Fam  ").unwrap(), "Fam");
        assert!(sanitize_group_name("   ").is_none());
    }
}

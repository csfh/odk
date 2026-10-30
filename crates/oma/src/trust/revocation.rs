//! Release keys a root has revoked (ADR 0060, section 3). A revoked key authorizes no new install
//! or upgrade, and a revocation is kept even when a later `oma-keys` omits the key or lists it
//! again. The document that revokes has no schema yet (`docs/oma-trust-plan.md`, question 2),
//! so nothing parses one: a [`Revocation`] is what a verifier of that document will return, and
//! until then only tests make one, from ids they take as already verified (issue #14).

use super::KeyId;

/// Release keys `root_key_id` has revoked, verified under that root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revocation {
    root_key_id: KeyId,
    ids: Vec<KeyId>,
}

impl Revocation {
    pub fn root_key_id(&self) -> &KeyId {
        &self.root_key_id
    }

    pub fn ids(&self) -> &[KeyId] {
        &self.ids
    }

    /// A revocation taken as verified: the tests' stand-in for a document that has no schema
    /// yet.
    #[cfg(test)]
    pub(crate) fn verified(root_key_id: KeyId, ids: Vec<KeyId>) -> Self {
        Self { root_key_id, ids }
    }
}

use std::fmt;

use rand::{self, RngExt};
pub struct PeerID(pub [u8; 20]);

impl PeerID {
    pub fn generate() -> Self {
        let mut id = [0u8; 20];
        rand::rng().fill(&mut id);
        Self(id)
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        bytes.try_into().ok().map(Self)
    }
}

impl fmt::Debug for PeerID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PeerId(")?;
        for byte in &self.0 {
            write!(f, "{:02x}", byte)?;
        }
        write!(f, ")")
    }
}

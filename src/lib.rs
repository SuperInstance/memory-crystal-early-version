pub mod crystal;
pub mod decay;
pub mod decoder;
pub mod encoder;
pub mod error;
pub mod index;
pub mod telephone;
pub mod tile;

// Re-export main types.
pub use crystal::{Crystal, CrystalStats};
pub use decay::DecaySchedule;
pub use decoder::{Reconstruction, TileDecoder};
pub use encoder::{ConstraintExtractor, TileEncoder};
pub use error::{CrystalError, Result};
pub use index::CrystalIndex;
pub use telephone::{FactTracker, TelephoneChain};
pub use tile::{SalienceMap, Tile, TileId};

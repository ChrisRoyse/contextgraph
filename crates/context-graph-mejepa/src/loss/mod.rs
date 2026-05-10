pub mod variance_floor;
pub mod vicreg;

pub use variance_floor::VarianceFloorHistory;
pub use vicreg::{huber_loss_delta_one, vicreg_loss};

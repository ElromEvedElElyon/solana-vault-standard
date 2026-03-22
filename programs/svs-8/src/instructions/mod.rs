//! Multi-asset vault instruction handlers.

pub mod admin;
pub mod add_asset;
pub mod deposit_proportional;
pub mod deposit_single;
pub mod initialize;
pub mod oracle_admin;
pub mod redeem_proportional;
pub mod redeem_single;
pub mod remove_asset;
pub mod update_weights;

#[allow(ambiguous_glob_reexports)]
pub use admin::*;
#[allow(ambiguous_glob_reexports)]
pub use add_asset::*;
#[allow(ambiguous_glob_reexports)]
pub use deposit_proportional::*;
#[allow(ambiguous_glob_reexports)]
pub use deposit_single::*;
#[allow(ambiguous_glob_reexports)]
pub use initialize::*;
#[allow(ambiguous_glob_reexports)]
pub use oracle_admin::*;
#[allow(ambiguous_glob_reexports)]
pub use redeem_proportional::*;
#[allow(ambiguous_glob_reexports)]
pub use redeem_single::*;
#[allow(ambiguous_glob_reexports)]
pub use remove_asset::*;
#[allow(ambiguous_glob_reexports)]
pub use update_weights::*;

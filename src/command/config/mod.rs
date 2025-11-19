mod autoinstall;
pub use autoinstall::run as autoinstall;

#[cfg(feature = "selfupdate")]
mod background_self_update;
#[cfg(feature = "selfupdate")]
pub use background_self_update::run as background_self_update;

#[cfg(feature = "selfupdate")]
mod modify_path;
#[cfg(feature = "selfupdate")]
pub use modify_path::run as modify_path;

#[cfg(feature = "selfupdate")]
mod startup_self_update;
#[cfg(feature = "selfupdate")]
pub use startup_self_update::run as startup_self_update;

#[cfg(not(windows))]
mod symlinks;
#[cfg(not(windows))]
pub use symlinks::run as symlinks;

mod versionsdb_update;
pub use versionsdb_update::run as versionsdb_update;

mod add;
pub use add::run as add;

mod api;
pub use api::run as api;

pub mod completions;
pub mod config;

mod default;
pub use default::run as default;

mod gc;
pub use gc::run as gc;

mod info;
pub use info::run as info;

mod initial_setup_from_launcher;
pub use initial_setup_from_launcher::run as initial_setup_from_launcher;

mod link;
pub use link::run as link;

mod list;
pub use list::run as list;

pub mod r#override;

mod remove;
pub use remove::run as remove;

#[cfg(feature = "selfupdate")]
mod selfchannel;
#[cfg(feature = "selfupdate")]
pub use selfchannel::run as selfchannel;

pub mod selfuninstall;
#[cfg(feature = "selfupdate")]
pub use selfuninstall::run as selfuninstall;

mod selfupdate;
pub use selfupdate::run as selfupdate;

mod status;
pub use status::run as status;

mod update;
pub use update::run as update;

mod update_versiondb;
pub use update_versiondb::run as update_versiondb;

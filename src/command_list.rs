use crate::{global_paths::GlobalPaths, versions_file::load_versions_db};
use anyhow::{Context,Result};
use cli_table::{ColorChoice,Table,format::{Border, Separator, HorizontalLine}, print_stdout, WithTitle};
use itertools::Itertools;

#[derive(Table)]
struct ChannelRow {
    #[table(title = "Channel")]
    name: String,
    #[table(title = "Version")]
    version: String,
}

pub fn run_command_list(_paths: &GlobalPaths) -> Result<()> {
    let versiondb_data =
        load_versions_db().with_context(|| "`list` command failed to load versions db.")?;

    let rows_in_table: Vec<_> = versiondb_data.available_channels
        .iter()
        .map(|i| -> ChannelRow {
            ChannelRow {
                name: i.0.to_string(),
                version: i.1.version.clone()
            }})
        .sorted_by_key(|i| i.name.clone())
        .collect();

    print_stdout(
        rows_in_table
        .with_title()
        .color_choice(ColorChoice::Never)
        .border(Border::builder().build())
        .separator(
            Separator::builder()
            .title(Some(HorizontalLine::new('1', '2', '3', '-')))
            .build()
        )
        
    )?;

    Ok(())
}

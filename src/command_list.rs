use crate::operations::{available_nightly_channels, nightlies_db_for_listing};
use crate::{global_paths::GlobalPaths, versions_file::load_versions_db};
use anyhow::{Context, Result};
use cli_table::{
    format::{Border, HorizontalLine, Separator},
    print_stdout, ColorChoice, Table, WithTitle,
};
use itertools::Itertools;
use numeric_sort::cmp;

#[derive(Table)]
struct ChannelRow {
    #[table(title = "Channel")]
    name: String,
    #[table(title = "Version")]
    version: String,
}

pub fn run_command_list(paths: &GlobalPaths) -> Result<()> {
    let versiondb_data =
        load_versions_db(paths).with_context(|| "`list` command failed to load versions db.")?;

    let nightlies = nightlies_db_for_listing(paths);
    let non_db_rows: Vec<ChannelRow> = available_nightly_channels(nightlies.as_ref())?
        .into_iter()
        .map(|(name, version)| ChannelRow { name, version })
        .collect();

    let rows_in_table: Vec<_> = versiondb_data
        .available_channels
        .iter()
        .map(|i| -> ChannelRow {
            ChannelRow {
                name: i.0.to_string(),
                version: i.1.version.clone(),
            }
        })
        .sorted_by(|a, b| cmp(&a.name, &b.name))
        .chain(non_db_rows)
        .collect();

    print_stdout(
        rows_in_table
            .with_title()
            .color_choice(ColorChoice::Never)
            .border(Border::builder().build())
            .separator(
                Separator::builder()
                    .title(Some(HorizontalLine::new('1', '2', '3', '-')))
                    .build(),
            ),
    )?;

    Ok(())
}

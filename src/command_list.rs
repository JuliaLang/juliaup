use crate::operations::{channel_to_name, compatible_archs};
use crate::{global_paths::GlobalPaths, versions_file::load_versions_db};
use anyhow::{Context, Result};
use cli_table::{
    format::{Border, HorizontalLine, Separator},
    print_stdout, ColorChoice, Table, WithTitle,
};
use human_sort::compare;
use itertools::Itertools;

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

    let non_db_channels: Vec<String> = std::iter::once("nightly".to_string())
        .chain(
            compatible_archs()?
                .into_iter()
                .map(|arch| format!("nightly~{}", arch)),
        )
        .chain(std::iter::once("pr{number}".to_string()))
        .collect();
    let non_db_rows: Vec<ChannelRow> = non_db_channels
        .into_iter()
        .map(|channel| {
            let name = channel_to_name(&channel).expect("Failed to identify version");
            ChannelRow {
                name: channel,
                version: name,
            }
        })
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
        .sorted_by(|a, b| compare(&a.name, &b.name))
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

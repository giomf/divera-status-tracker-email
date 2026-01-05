use clap::Parser;
use cli::Cli;
use std::{fs, path::Path};

mod cli;
mod data;
mod fetcher;
mod io;

const OUTPUT: &str = "output";
const ATTACHMENTS: &str = "attachments";
const ON_DUTY_NAME: &str = "on-duty.xlsx";
const HISTORY_NAME: &str = "history.html";

fn main() {
    let cli = Cli::parse();
    let output_folder = Path::new(OUTPUT);
    let attachments_path = output_folder.join(ATTACHMENTS);
    let aggregation_path = output_folder.join("aggregation.parquet");
    fs::create_dir_all(&attachments_path).expect("Failed to create attachments folder");

    match cli {
        Cli::Fetch(cmd) => {
            fetcher::fetch_subject_messages(
                cmd.host,
                cmd.email,
                cmd.password,
                cmd.subject,
                &attachments_path,
            );
        }
        Cli::Update(cmd) => {
            fetcher::fetch_subject_messages(
                cmd.host,
                cmd.email,
                cmd.password,
                cmd.subject,
                &attachments_path,
            );
            let aggregation_data =
                data::aggregate_attachments(&attachments_path, &cmd.off_duty_keyword);
            io::write_parquet(aggregation_data, &aggregation_path);
        }
        Cli::Aggregate(cmd) => {
            let aggregation_data =
                data::aggregate_attachments(&attachments_path, &cmd.off_duty_keyword);
            io::write_parquet(aggregation_data, &aggregation_path);
        }

        Cli::OnDuty(cmd) => {
            let aggregation_data = io::read_parquet(&aggregation_path);
            let on_duty = data::calculate(aggregation_data);
            if cmd.print {
                println!("{on_duty}");
            }
            if cmd.export {
                let export_path = output_folder.join(ON_DUTY_NAME);
                io::write_excel(&export_path, &on_duty);
                println!(
                    "On-duty exported to {}",
                    export_path.to_string_lossy().to_string()
                )
            }
        }

        Cli::History(cmd) => {
            let aggregation_data = io::read_parquet(&aggregation_path);
            let history = data::history(aggregation_data, 12, cmd.year, cmd.month);

            if cmd.print {
                println!("{history}");
            }

            if cmd.export {
                let export_path = output_folder.join(HISTORY_NAME);
                io::plot_history(history, &export_path);
                println!(
                    "History exported to {}",
                    export_path.to_string_lossy().to_string()
                )
            }
        }
    };
}

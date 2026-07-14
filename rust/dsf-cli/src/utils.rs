use colored::Colorize;
use dsf_core::core::{DataSetBusyStatus, DataSetStatus};
pub(crate) fn print_query(id: &str, status: DataSetStatus, dep_statuses: &[DataSetStatus]) {
    let s = fmt_query(status);
    println!("dataset: {}", id.cyan());
    println!("status:  {}", s);

    if dep_statuses.is_empty() {
        println!("deps:    []");
    } else {
        let rendered: Vec<String> = dep_statuses.iter().map(|s| fmt_query(*s)).collect();
        println!("deps:    [{}]", rendered.join(", "));
    }
}

pub(crate) fn fmt_query(s: DataSetStatus) -> String {
    match s {
        DataSetStatus::Healthy => "Healthy".green().to_string(),
        DataSetStatus::Broken => "Broken".red().to_string(),
        DataSetStatus::BrokenDeps => "BrokenDeps".yellow().to_string(),
        DataSetStatus::Unverified => "Unverified".normal().to_string(),

        DataSetStatus::Busy(busy) => match busy {
            DataSetBusyStatus::Reading => format!("Occupied({})", busy.as_str().cyan()),
            DataSetBusyStatus::Modifying | DataSetBusyStatus::Creating => {
                format!("Occupied({})", busy.as_str().magenta())
            }
            DataSetBusyStatus::Deleting => format!("Occupied({})", busy.as_str().red()),
        },
    }
}

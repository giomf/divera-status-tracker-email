use std::path::Path;

use crate::io;
use chrono::NaiveDate;
use either::Either;
use glob::glob;
use polars::lazy::prelude::*;
use polars::prelude::*;

const NAME_COLUMN_NAME: &str = "Name";
pub const DATE_COLUMN_NAME: &str = "Date";
pub const TOTAL_ON_DUTY_COLUMN_NAME: &str = "Total On-Duty";
const WEEKEND_ON_DUTY_COLUMN_NAME: &str = "Weekend On-Duty";
const OFF_DUTY_WITH_NOTE_COLUMN_NAME: &str = "Off-Duty with Note";
const WEEKEND_BEGIN_HOUR: i8 = 18;
const YEAR_MONTHS: u32 = 12;
const WEEKDAY_FRIDAY: u32 = 5;

lazy_static::lazy_static! {
    static ref DATES: Expr = col(DATE_COLUMN_NAME);
    static ref NAMES: Expr = col("*").exclude([DATE_COLUMN_NAME]);
    static ref STATE: Expr = NAMES.clone().struct_().field_by_name("state");
    static ref NOTE: Expr = NAMES.clone().struct_().field_by_name("note");
    static ref COUNT: Expr = NAMES.clone().count();
}

pub fn aggregate_attachments(attachments_path: &Path, off_duty_keyword: &str) -> DataFrame {
    glob(
        attachments_path
            .join("*.xlsx")
            .as_os_str()
            .to_str()
            .unwrap(),
    )
    .expect("Failed to read glob pattern")
    .map(|path| io::read_excel(&path.unwrap(), off_duty_keyword))
    .reduce(|acc, df| {
        acc.clone()
            .lazy()
            .join(
                df.clone().lazy(),
                [col("name")],
                [col("name")],
                JoinArgs::new(JoinType::Full).with_coalesce(JoinCoalesce::CoalesceColumns),
            )
            .collect()
            .unwrap()
    })
    .unwrap()
    .transpose(
        Some(DATE_COLUMN_NAME),
        Some(Either::Left("name".to_string())),
    )
    .unwrap()
    .lazy()
    .select([
        DATES
            .clone()
            .str()
            .to_datetime(None, None, StrptimeOptions::default(), lit("raise")),
        NAMES.clone(),
    ])
    .collect()
    .unwrap()
}

pub fn history(df: DataFrame, minimum: u32, year: Option<i32>, month: Option<u32>) -> DataFrame {
    let history = df.clone().lazy();

    let history = if let (Some(year), Some(month)) = (year, month) {
        let (to_year, to_month) = if month == YEAR_MONTHS {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };

        let from_date = NaiveDate::from_ymd_opt(year, month, 1).expect("Unable to create date");
        let to_date = NaiveDate::from_ymd_opt(to_year, to_month, 1).expect("Unable to create date");
        history.filter(
            DATES
                .clone()
                .gt(lit(from_date))
                .and(DATES.clone().lt(lit(to_date))),
        )
    } else {
        history
    };

    history
        .select([
            DATES.clone(),
            sum_horizontal([STATE.clone()])
                .unwrap()
                .alias(TOTAL_ON_DUTY_COLUMN_NAME),
        ])
        .with_column(lit(minimum).alias("Minimum"))
        .collect()
        .unwrap()
}

pub fn calculate(df: DataFrame) -> DataFrame {
    let total_on_duty = calculate_total_on_duty(&df);
    let weekend_on_duty = calculate_weekend_on_duty(&df);
    let off_duty_with_note = calculate_off_duty_with_note(&df);

    let mut result = total_on_duty
        .clone()
        .lazy()
        .join(
            weekend_on_duty.lazy(),
            [col("name")],
            [col("name")],
            JoinArgs::new(JoinType::Left),
        )
        .join(
            off_duty_with_note.lazy(),
            [col("name")],
            [col("name")],
            JoinArgs::new(JoinType::Left),
        )
        .collect()
        .unwrap();

    let result = result.rename("name", NAME_COLUMN_NAME).unwrap().clone();

    let sort_options = SortMultipleOptions::new()
        .with_multithreaded(true)
        .with_order_descending(true);

    result
        .sort([TOTAL_ON_DUTY_COLUMN_NAME], sort_options)
        .unwrap()
}

fn calculate_total_on_duty(df: &DataFrame) -> DataFrame {
    let mut total_on_duty = df
        .clone()
        .lazy()
        .select([
            (STATE.clone().sum().cast(DataType::Float32) / STATE.clone().count() * lit(100))
                .name()
                .keep(),
        ])
        .collect()
        .unwrap()
        .transpose(Some("name"), None)
        .unwrap();
    total_on_duty
        .rename("column_0", TOTAL_ON_DUTY_COLUMN_NAME)
        .unwrap()
        .clone()
}

fn calculate_weekend_on_duty(df: &DataFrame) -> DataFrame {
    let weekend = DATES
        .clone()
        .dt()
        .weekday()
        .gt(lit(WEEKDAY_FRIDAY))
        .or(DATES
            .clone()
            .dt()
            .weekday()
            .eq(lit(WEEKDAY_FRIDAY))
            .and(DATES.clone().dt().hour().gt_eq(lit(WEEKEND_BEGIN_HOUR))));

    let weekend_on_duty = STATE.clone().and(weekend.clone());
    let active_weekend_on_duty = STATE.clone().is_not_null().and(weekend.clone());

    let mut weekend_on_duty =
        df.clone()
            .lazy()
            .select([(weekend_on_duty.sum().cast(DataType::Float32)
                / active_weekend_on_duty.sum()
                * lit(100))
            .name()
            .keep()])
            .collect()
            .unwrap()
            .transpose(Some("name"), None)
            .unwrap();

    weekend_on_duty
        .rename("column_0", WEEKEND_ON_DUTY_COLUMN_NAME)
        .unwrap()
        .clone()
}

fn calculate_off_duty_with_note(df: &DataFrame) -> DataFrame {
    let off_duty_with_note = STATE.clone().not().and(NOTE.clone()).sum();

    let mut off_duty_with_note =
        df.clone()
            .lazy()
            .select([(off_duty_with_note.cast(DataType::Float32)
                / STATE.clone().xor(lit(true)).sum()
                * lit(100))
            .name()
            .keep()])
            .collect()
            .unwrap()
            .transpose(Some("name"), None)
            .unwrap();

    off_duty_with_note
        .rename("column_0", OFF_DUTY_WITH_NOTE_COLUMN_NAME)
        .unwrap()
        .clone()
}

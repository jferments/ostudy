use chrono::NaiveDate;

use super::study_date;

#[test]
fn an_hour_before_the_new_day_belongs_to_the_previous_study_date() {
    let date = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
    let previous = NaiveDate::from_ymd_opt(2026, 2, 28).unwrap();
    assert_eq!(study_date(date, 3, 4), previous);
    assert_eq!(study_date(date, 4, 4), date);
    assert_eq!(study_date(date, 0, 0), date);
    assert_eq!(study_date(date, 22, 23), previous);
}

mod calendar;

fn main() {
    println!("stars - version 0.1.0");
    println!("2024 is a leap year: {}", calendar::is_leap_year(2024));
    println!(
        "Days in February 2024: {}",
        calendar::days_in_month(2024, 2)
    );
    println!(
        "2024-02-29 is valid: {}",
        calendar::is_valid_day(2024, 2, 29)
    );
    println!("Month labels: {:?}", calendar::MONTH_LABELS);
}

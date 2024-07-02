use chrono::Datelike;
use chrono::NaiveDate;
use chrono::Utc;
use chrono::Timelike;
use holochain_types::prelude::ClonedCell;

pub const BUCKET_SIZE_DAYS: u32 = 14;
pub const HOLO_EPOCH_YEAR: u16 = 2024;

pub fn build_json_sl_props(bound_happ_id: &str,bound_hha_dna:&str, bound_hf_dna:&str, holo_admin: &str, bucket_size: u32, time_bucket: u32) -> String {
    format!(
        r#"{{"bound_happ_id":"{}", "bound_hha_dna":"{}", "bound_hf_dna":"{}", "holo_admin": "{}", "bucket_size": {}, "time_bucket": {}}}"#,
        bound_happ_id,
        bound_hha_dna,
        bound_hf_dna,
        holo_admin,
        bucket_size,
        time_bucket,
    )
}

pub fn get_current_time_bucket(days_in_bucket: u32) -> u32 {
    let now_utc = Utc::now();
    if std::env::var("IS_TEST_ENV").is_ok() {
        10
    } else {
        let epoch_start = NaiveDate::from_ymd_opt(HOLO_EPOCH_YEAR.into(), 1, 1).unwrap();
        let days_since_epoch : u32 = (now_utc.num_days_from_ce()-epoch_start.num_days_from_ce()).try_into().expect("now should always be after Holo epoch");
        days_since_epoch/days_in_bucket
    }
}


pub fn get_service_logger_bucket_range(_clone_cells: Vec<ClonedCell>, days: u32) -> (u32, u32, u32){
    let bucket_size = BUCKET_SIZE_DAYS; // TODO: get this from: clone_cells[0].dna_modifiers.properties;
    let time_bucket: u32 = get_current_time_bucket(bucket_size);
    let buckets_for_days_in_request = days/bucket_size;
    (bucket_size, time_bucket, buckets_for_days_in_request)
}
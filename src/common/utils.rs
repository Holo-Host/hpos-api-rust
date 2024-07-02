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


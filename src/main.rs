use cloud_storage::client::Client;
use goose::{prelude::*, config};
use std::{result::Result, error::Error, fs};

const TMP_REPORT_FILE: &'static str = "/tmp/report.html";

async fn loadtest_hello(user: &mut GooseUser) -> TransactionResult {
    let _goose_metrics = user.get("/hello").await?;
    let _goose_metrics = user.get("/hello?name=world").await?;
    let _goose_metrics = user.get("/hello?name=supercalifragilisticexpialidocious%20supercalifragilisticexpialidocious%20supercalifragilisticexpialidocious%20supercalifragilisticexpialidocious%20supercalifragilisticexpialidocious").await?;

    Ok(())
}

async fn loadtest_static(user: &mut GooseUser) -> TransactionResult {
    let _goose_metrics = user.get("/static/basic.html").await?;
    let _goose_metrics = user.get("/static/scout.webp").await?;

    Ok(())
}

async fn maybe_copy_to_gcs(maybe_bucket_name: Option<String>, maybe_filename: Option<String>) -> Result<(), Box<dyn Error>> {
    let bucket_name = match maybe_bucket_name {
        Some(b) => b,
        _ => return Ok(())
    };
    let filename = match maybe_filename {
        Some(f) => f,
        _ => return Ok(())
    };
    if bucket_name.is_empty() || filename.is_empty() {
        return Ok(())
    }

    let f = fs::read(TMP_REPORT_FILE)?;

    let client = Client::new();
    client.object().create(bucket_name.as_str(), f, filename.as_str(), "text/html").await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Parsing load test args");
    let options = xflags::parse_or_exit! {
        /// Host to load test.
        required --host h: String
        /// The number of users to hatch.
        required --users u: usize
        /// The start time over which users are hatched (eg. 10s, 20m).
        required --start_time st: String
        /// The steady state run time, after all users are hatched (eg. 10s, 20m).
        required --run_time st: String
        /// The GCS bucket to write an HTML report file to, if any.
        optional --bucket b: String
        /// The filename for the HTML report in the GCS bucket (see --bucket).
        optional --report_name rn: String
    };

    let mut configuration = config::GooseConfiguration::default();
    configuration.host = options.host;
    configuration.report_file = TMP_REPORT_FILE.to_string();
    // Max 10 second timeout per request.
    configuration.timeout = Some("10".to_string());
    configuration.users = Some(options.users);
    configuration.startup_time = options.start_time;
    configuration.run_time = options.run_time;

    GooseAttack::initialize_with_config(configuration)?
        .register_scenario(scenario!("LoadtestTransactions")
            .register_transaction(transaction!(loadtest_hello))
            .register_transaction(transaction!(loadtest_static))
        )
        .execute()
        .await?;

    maybe_copy_to_gcs(options.bucket, options.report_name).await?;

    Ok(()) 
}
